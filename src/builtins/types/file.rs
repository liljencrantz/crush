use crate::data::binary::BinaryReader;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::errors::{CrushResult, command_error, data_error, error};
use crate::lang::pipe::TableOutputStream;
use crate::lang::signature::text::Text;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::user_map::{get_gid, get_uid};
use nix::errno::Errno;
use nix::fcntl::AT_FDCWD;
use nix::libc::S_IFDIR;
use nix::sys::stat::{UtimensatFlags, lstat, utimensat};
use nix::sys::time::TimeSpec;
use ordered_map::OrderedMap;
use signature::signature;
use std::collections::HashSet;
use std::fs::{File, create_dir, metadata, remove_dir, remove_file};
use std::ops::{Add, Deref};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use crate::lang::value::Value::Empty;
use crate::util::directory_lister::{DirectoryLister, directory_lister};
use crate::util::file_remover::{FileRemover, file_remover};

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        Chown::declare_method(&mut res);
        Chmod::declare_method(&mut res);
        Exists::declare_method(&mut res);
        GetItem::declare_method(&mut res);
        Write::declare_method(&mut res);
        Read::declare_method(&mut res);
        Parent::declare_method(&mut res);
        Name::declare_method(&mut res);
        Remove::declare_method(&mut res);
        MkDir::declare_method(&mut res);
        Touch::declare_method(&mut res);
        res
    })
}

#[signature(
    types.file.chown,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Change owner of this file.",
)]
struct Chown {
    #[description("the owning user for the file.")]
    user: Option<String>,
    #[description("the owning group for the file.")]
    group: Option<String>,
}

pub fn chown(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Chown::parse(context.remove_arguments(), &context.global_state.printer())?;
    let file = context.this.file()?;

    let uid = if let Some(name) = cfg.user {
        Some(get_uid(&name)?.ok_or(format!("Unknown user {}", &name))?)
    } else {
        None
    };

    let gid = if let Some(name) = cfg.group {
        Some(get_gid(&name)?.ok_or(format!("Unknown group {}", &name))?)
    } else {
        None
    };

    nix::unistd::chown(
        &file,
        uid.map(|i| i.add(0).into()),
        gid.map(|i| i.add(0).into()),
    )?;

    context.output.send(Value::Empty)
}

#[signature(
    types.file.chmod,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Change permissions of this file.",
    long = "Permissions are strings of the form [classes...][adjustment][modes..].",
    long = "* A class is one of u, g, o, a, signifying file owner, file group, other users and all users, respectively.",
    long = "* The adjustment must be one of +, -, and =, signifying added permissions, removed permissions and set permissions, respectively.",
    long = "* A mode is one of r w, x, signifying read, write and execute permissions.",
    example = "'./foo'.chmod \"a=\" \"u+r\" # First strip all rights for all users, then re-add read rights for the owner",
)]
struct Chmod {
    #[description("the set of permissions to add.")]
    #[unnamed()]
    permissions: Vec<String>,
}

const OWNER: u32 = 6;
const GROUP: u32 = 3;
const OTHER: u32 = 0;

enum PermissionAdjustment {
    Add,
    Remove,
    Set,
}

const READ: u32 = 4;
const WRITE: u32 = 2;
const EXECUTE: u32 = 1;

fn apply(perm: &str, mut current: u32) -> CrushResult<u32> {
    let mut class_done = false;
    let mut classes = HashSet::new();
    let mut adjustments = PermissionAdjustment::Add;
    let mut modes = 0u32;

    for c in perm.chars() {
        match class_done {
            false => match c {
                'u' => {
                    classes.insert(OWNER);
                }
                'g' => {
                    classes.insert(GROUP);
                }
                'o' => {
                    classes.insert(OTHER);
                }
                'a' => {
                    classes.insert(OWNER);
                    classes.insert(GROUP);
                    classes.insert(OTHER);
                }
                '+' => {
                    class_done = true;
                }
                '-' => {
                    adjustments = PermissionAdjustment::Remove;
                    class_done = true;
                }
                '=' => {
                    adjustments = PermissionAdjustment::Set;
                    class_done = true;
                }
                c => {
                    return command_error(format!(
                        "Illegal character in class-part of permission: {}",
                        c
                    ));
                }
            },
            true => match c {
                'r' => modes |= READ,
                'w' => modes |= WRITE,
                'x' => modes |= EXECUTE,
                c => {
                    return command_error(format!(
                        "Illegal character in mode-part of permission: {}.",
                        c
                    ));
                }
            },
        }
    }

    if !class_done {
        return command_error("Premature end of permission.");
    }

    if classes.is_empty() {
        return command_error("No user classes specified in permission.");
    }

    for cl in classes {
        match adjustments {
            PermissionAdjustment::Add => {
                // Add new bits
                current |= modes << cl;
            }
            PermissionAdjustment::Remove => {
                // Remove bits
                current = current & !(modes << cl);
            }
            PermissionAdjustment::Set => {
                // Clear current bits
                current = current & !(7 << cl);
                // Add new bits
                current |= modes << cl;
            }
        }
    }

    Ok(current)
}

pub fn chmod(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Chmod::parse(context.remove_arguments(), &context.global_state.printer())?;
    let file = context.this.file()?;
    let metadata = metadata(&file)?;

    let mut current: u32 = metadata.permissions().mode();

    for perm in cfg.permissions {
        current = apply(&perm, current)?;
    }

    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(current))?;
    context.output.send(Value::Empty)
}

#[signature(
    types.file.exists,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if the file exists.",
)]
struct Exists {}

pub fn exists(mut context: CommandContext) -> CrushResult<()> {
    context
        .output
        .send(Value::Bool(context.this.file()?.exists()))
}

#[signature(
    types.file.__getitem__,
    can_block = false,
    output = Known(ValueType::File),
    short = "Return a file or subdirectory in the specified base directory.",
    example = "$filename := foo.txt",
    example = "$base_directory := .",
    example = "$file := $base_directory[$filename]",
)]
struct GetItem {
    #[description("the name of the file or subdirectory")]
    name: Text,
}

pub fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let base_directory = context.this.file()?;
    let cfg = GetItem::parse(context.remove_arguments(), &context.global_state.printer())?;
    context
        .output
        .send(Value::from(base_directory.join(&cfg.name.as_path())))
}

#[signature(
    types.file.write,
    can_block = true,
    output = Known(ValueType::Empty),
    short = "Write a binary_stream to this file. If no stream is given, input pipe must be one.",
)]
struct Write {}

fn write(mut context: CommandContext) -> CrushResult<()> {
    let _cfg = Write::parse(context.remove_arguments(), &context.global_state.printer())?;
    match context.input.recv()? {
        Value::BinaryInputStream(mut input) => {
            let mut out = File::create(context.this.file()?)?;
            std::io::copy(input.as_mut(), &mut out)?;
            Ok(())
        }
        _ => command_error("Expected a binary stream."),
    }
}

#[signature(
    types.file.read,
    can_block = true,
    output = Known(ValueType::BinaryInputStream),
    short = "A read source for binary_stream values",
)]
struct Read {}

fn read(mut context: CommandContext) -> CrushResult<()> {
    context
        .output
        .send(Value::BinaryInputStream(<dyn BinaryReader>::paths(vec![
            context.this.file()?,
        ])?))
}

#[signature(
    types.file.name,
    can_block = false,
    output = Known(ValueType::String),
    short = "The name (excluding path) of this file, as a string",
)]
struct Name {}

fn name(mut context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::from(
        context
            .this
            .file()?
            .file_name()
            .ok_or("Invalid file path.")?
            .to_str()
            .ok_or("Invalid file name.")?,
    ))
}

#[signature(
    types.file.parent,
    can_block = false,
    output = Known(ValueType::File),
    short = "The parent directory of this file",
)]
struct Parent {}

fn parent(mut context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::from(
        context.this.file()?.parent().ok_or("Invalid file path.")?,
    ))
}

static REMOVE_OUTPUT_TYPE: [ColumnType; 3] = [
    ColumnType::new("file", ValueType::File, "the file this row describes."),
    ColumnType::new("deleted", ValueType::Bool, "true if the file was successfully deleted."),
    ColumnType::new("status", ValueType::String, "\"Deleted\", or the error that prevented deletion."),
];

#[signature(
    types.file.remove,
    can_block = true,
    output = Known(ValueType::table_input_stream(& REMOVE_OUTPUT_TYPE)),
    short = "Delete this file",
    long = "Returns a stream of deletion failures."
)]
struct Remove {
    #[description("If this file is a directory, recursively delete files and subdirectories")]
    #[default(false)]
    recursive: bool,
    #[description("If true, emit status updates for deleted files, not just errors")]
    #[default(false)]
    verbose: bool,
}

fn remove_outcome_to_row<ErrType: ToString>(path: Arc<Path>, result: Result<(), ErrType>) -> Row {
    match result {
        Ok(_) => Row::new(vec![
            Value::File(path),
            Value::Bool(true),
            Value::String(Arc::from("Deleted")),
        ]),
        Err(e) => Row::new(vec![
            Value::File(path),
            Value::Bool(false),
            Value::String(Arc::from(e.to_string())),
        ]),
    }
}

fn handle_remove_result<ErrType: ToString>(
    path: Arc<Path>,
    result: Result<(), ErrType>,
    out: &TableOutputStream,
    verbose: bool,
) -> CrushResult<()> {
    match result {
        Ok(_) => {
            if verbose {
                out.send(remove_outcome_to_row::<String>(path, Ok(())))
            } else {
                Ok(())
            }
        }
        Err(e) => out.send(remove_outcome_to_row(path, Err(e))),
    }
}

fn remove_file_of_unknown_type(
    path: Arc<Path>,
    out: &TableOutputStream,
    verbose: bool,
) -> CrushResult<()> {
    match lstat(path.deref()) {
        Ok(stat) => {
            if stat.st_mode & S_IFDIR != 0 {
                let res = remove_dir(path.deref());
                handle_remove_result(path, res, out, verbose)
            } else {
                let res = remove_file(path.deref());
                handle_remove_result(path, res, out, verbose)
            }
        }
        Err(e) => handle_remove_result(path, Err(e), out, verbose),
    }
}

// Generic over `DirectoryLister`/`FileRemover` (same fake-for-tests/real-for-the-shell split
// `glob_files` uses, see src/util/directory_lister.rs) so the interruption behavior below can
// be verified by a deterministic unit test, with no real filesystem, timing, or job control
// involved: see the `tests` module at the bottom of this file.
fn remove_recursive(
    root: Arc<Path>,
    out: &TableOutputStream,
    verbose: bool,
    lister: &impl DirectoryLister,
    remover: &impl FileRemover,
) -> CrushResult<()> {
    let mut directories = vec![(root, false)];

    while let Some((next, subdirectories_already_deleted)) = directories.pop() {
        if subdirectories_already_deleted {
            let res = remover.remove_dir(&next);
            handle_remove_result(next, res, out, verbose)?;
        } else {
            directories.push((next.clone(), true));
            match lister.list(next.to_path_buf()) {
                Ok(entries) => {
                    for e in entries {
                        // `output` is already interruptible (`CommandContext::initialize_output`
                        // wires its controller up), and `send` already checks it on every call --
                        // but a file that isn't itself reported (the default, non-`verbose` case)
                        // never reaches a `send` at all, so this walk needs its own check between
                        // entries, same gap `fs:usage` had (see that fix's own commit).
                        out.poll_control()?;
                        if e.is_directory {
                            directories.push((Arc::from(e.full_path.as_path()), false));
                        } else {
                            let path: Arc<Path> = Arc::from(e.full_path.as_path());
                            let res = remover.remove_file(&path);
                            handle_remove_result(path, res, out, verbose)?;
                        }
                    }
                }
                Err(e) => out.send(remove_outcome_to_row(next.clone(), Err(e)))?,
            }
        }
    }
    Ok(())
}

fn remove(mut context: CommandContext) -> CrushResult<()> {
    let output = context.initialize_output(&REMOVE_OUTPUT_TYPE)?;
    let cfg = Remove::parse(context.remove_arguments(), context.global_state.printer())?;
    match context.this {
        Some(Value::File(file)) => {
            if cfg.recursive {
                remove_recursive(
                    file,
                    &output,
                    cfg.verbose,
                    &directory_lister(),
                    &file_remover(),
                )
            } else {
                remove_file_of_unknown_type(file, &output, cfg.verbose)
            }
        }
        None => command_error("Expected `this` to be a `file`, but it was not set."),
        Some(v) => command_error(&format!(
            "Expected `this` to be of type `file`, but is of type `{}`.",
            v.value_type()
        )),
    }
}

#[signature(
    types.file.mkdir,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Create directory",
)]
struct MkDir {
    #[description("Do not throw and error if this directory already exists.")]
    #[default(false)]
    ignore_existing: bool,
}

fn mkdir_recursive(path: &Path, leaf: bool, ignore_existing: bool) -> CrushResult<()> {
    if path.exists() && path.is_dir() {
        if leaf && !ignore_existing {
            data_error(format!("Directory `{}` already exists.", path.display()))
        } else {
            Ok(())
        }
    } else {
        if let Some(parent) = path.parent() {
            mkdir_recursive(parent, false, ignore_existing)?;
        }
        Ok(create_dir(path)?)
    }
}

fn mkdir(mut context: CommandContext) -> CrushResult<()> {
    let cfg = MkDir::parse(context.remove_arguments(), context.global_state.printer())?;
    let directory = context.this.file()?;
    mkdir_recursive(&directory, true, cfg.ignore_existing)?;
    context.output.send(Empty)
}

#[signature(
    types.file.touch,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Set the modification and access times of file.",
    long = "If the file doesn't exist, it is created.",
)]
struct Touch {
    #[description("Do not create the file if it doesn't exist.")]
    #[default(false)]
    no_create: bool,
}

fn touch(mut context: CommandContext) -> CrushResult<()> {
    let file = context.this.file()?;
    let cfg = Touch::parse(context.remove_arguments(), context.global_state.printer())?;
    match utimensat(
        AT_FDCWD,
        &file,
        &TimeSpec::UTIME_NOW,
        &TimeSpec::UTIME_NOW,
        UtimensatFlags::FollowSymlink,
    ) {
        Ok(_) => context.output.send(Empty),
        Err(Errno::ENOENT) => {
            if !cfg.no_create {
                File::create_new(file)?;
            }
            context.output.send(Empty)
        }
        Err(err) => error(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::pipe::unlimited_streams;
    use crate::util::directory_lister::tests::FakeDirectoryLister;
    use crate::util::file_remover::tests::FakeFileRemover;
    use std::path::PathBuf;

    // /root
    // |- a       (file)
    // `- sub     (dir)
    //    `- b    (file)
    fn fake_tree() -> FakeDirectoryLister {
        let mut lister = FakeDirectoryLister::new("/");
        lister.add("/root/sub", &vec!["b"]);
        lister.add("/root", &vec!["a"]);
        lister
    }

    // Deliberately synchronous and deterministic, no real filesystem, pty, subprocess, job
    // control, or timing involved: a Terminate message is queued on the output's control
    // channel *before* remove_recursive is ever called, so the very first poll_control() call
    // inside the walk -- which happens before the first entry is even looked at, see that call
    // site's own comment -- is guaranteed to observe it. This is the same race-free pattern
    // src/lang/pipe.rs's own
    // `plain_clone_of_an_interruptible_table_output_stream_does_not_broadcast_control` test
    // uses. (An end-to-end version of this property used to be tested via a real 100,000-file
    // pty session racing `terminate` against the delete finishing -- replaced by this test,
    // since that approach could never fully remove the possibility of flaking under load; see
    // this commit.)
    #[test]
    fn recursive_remove_stops_before_touching_anything_when_already_terminated() {
        let lister = fake_tree();
        let remover = FakeFileRemover::new();
        let (output, controller) = unlimited_streams(vec![]).0.interruptible();
        controller.terminate().unwrap();

        let result = remove_recursive(
            Arc::from(Path::new("/root")),
            &output,
            false,
            &lister,
            &remover,
        );

        assert!(
            result.is_err(),
            "expected a Terminate already queued before the walk starts to abort it"
        );
        assert_eq!(
            remover.removed(),
            Vec::<PathBuf>::new(),
            "expected nothing to have been removed: a Terminate queued before the walk starts \
             must be observed on the very first poll_control() call, before the first entry is \
             even looked at"
        );
    }

    #[test]
    fn recursive_remove_removes_every_entry_when_uninterrupted() {
        let lister = fake_tree();
        let remover = FakeFileRemover::new();
        let (output, _controller) = unlimited_streams(vec![]).0.interruptible();

        let result = remove_recursive(
            Arc::from(Path::new("/root")),
            &output,
            false,
            &lister,
            &remover,
        );

        assert!(result.is_ok(), "expected an uninterrupted walk to succeed: {:?}", result);

        let mut removed = remover.removed();
        removed.sort();
        let mut expected = vec![
            PathBuf::from("/root/a"),
            PathBuf::from("/root/sub/b"),
            PathBuf::from("/root/sub"),
            PathBuf::from("/root"),
        ];
        expected.sort();
        assert_eq!(removed, expected);
    }
}
