use crate::data::binary::BinaryReader;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::errors::{CrushResult, argument_error_legacy, data_error, error};
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
    let cfg = Chown::parse(context.arguments, &context.global_state.printer())?;
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
    example = "./foo:chmod \"a=\" \"u+r\" # First strip all rights for all users, then re-add read rights for the owner",
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
                    return argument_error_legacy(format!(
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
                    return argument_error_legacy(format!(
                        "Illegal character in mode-part of permission: {}",
                        c
                    ));
                }
            },
        }
    }

    if !class_done {
        return argument_error_legacy("Premature end of permission");
    }

    if classes.is_empty() {
        return argument_error_legacy("No user classes specified in permission");
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
    let cfg = Chmod::parse(context.arguments, &context.global_state.printer())?;
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
    let cfg = GetItem::parse(context.arguments, &context.global_state.printer())?;
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
    let _cfg = Write::parse(context.arguments, &context.global_state.printer())?;
    match context.input.recv()? {
        Value::BinaryInputStream(mut input) => {
            let mut out = File::create(context.this.file()?)?;
            std::io::copy(input.as_mut(), &mut out)?;
            Ok(())
        }
        _ => argument_error_legacy("Expected a binary stream"),
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
            .ok_or("Invalid file path")?
            .to_str()
            .ok_or("Invalid file name")?,
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
        context.this.file()?.parent().ok_or("Invalid file path")?,
    ))
}

static REMOVE_OUTPUT_TYPE: [ColumnType; 3] = [
    ColumnType::new("file", ValueType::File),
    ColumnType::new("deleted", ValueType::Bool),
    ColumnType::new("status", ValueType::String),
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

fn remove_known_directory(
    path: Arc<Path>,
    out: &TableOutputStream,
    verbose: bool,
) -> CrushResult<()> {
    let res = remove_dir(path.deref());
    handle_remove_result(path, res, out, verbose)
}

fn remove_known_file(path: Arc<Path>, out: &TableOutputStream, verbose: bool) -> CrushResult<()> {
    let res = remove_file(path.deref());
    handle_remove_result(path, res, out, verbose)
}

fn remove(mut context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(&REMOVE_OUTPUT_TYPE)?;
    let cfg = Remove::parse(context.remove_arguments(), context.global_state.printer())?;
    match context.this {
        Some(Value::File(file)) => {
            if cfg.recursive {
                let mut directories = vec![(file, false)];

                while let Some((next, subdirectories_already_deleted)) = directories.pop() {
                    if subdirectories_already_deleted {
                        remove_known_directory(next, &output, cfg.verbose)?;
                    } else {
                        directories.push((next.clone(), true));
                        match next.clone().read_dir() {
                            Ok(rd) => {
                                for e in rd {
                                    match e {
                                        Ok(e) => match e.metadata() {
                                            Ok(meta) => {
                                                if meta.is_dir() {
                                                    directories.push((Arc::from(e.path()), false));
                                                } else {
                                                    remove_known_file(
                                                        Arc::from(e.path()),
                                                        &output,
                                                        cfg.verbose,
                                                    )?;
                                                }
                                            }
                                            Err(e) => output.send(remove_outcome_to_row(
                                                next.clone(),
                                                Err(e),
                                            ))?,
                                        },

                                        Err(e) => output
                                            .send(remove_outcome_to_row(next.clone(), Err(e)))?,
                                    }
                                }
                            }
                            Err(e) => output.send(remove_outcome_to_row(next.clone(), Err(e)))?,
                        }
                    }
                }
                Ok(())
            } else {
                remove_file_of_unknown_type(file, &output, cfg.verbose)
            }
        }
        None => argument_error_legacy("Expected this to be a file, but this is not set"),
        Some(v) => argument_error_legacy(&format!(
            "Expected this to be a file, but it is a {}",
            v.value_type().to_string()
        )),
    }
}

#[signature(
    types.file.mkdir,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Create directory",
)]
struct MkDir {}

fn mkdir_recursive(path: &Path, leaf: bool) -> CrushResult<()> {
    if path.exists() && path.is_dir() {
        if leaf {
            data_error("Directory already exists")
        } else {
            Ok(())
        }
    } else {
        if let Some(parent) = path.parent() {
            mkdir_recursive(parent, false)?;
        }
        Ok(create_dir(path)?)
    }
}

fn mkdir(mut context: CommandContext) -> CrushResult<()> {
    let directory = context.this.file()?;
    mkdir_recursive(&directory, true)
}

#[signature(
    types.file.touch,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Set the modification and access times of file.",
    long = "If the file doesn't exist, it is created.",
)]
struct Touch {
    #[description("Do not create the file if it doesn't exist")]
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
        Ok(_) => Ok(()),
        Err(Errno::ENOENT) => {
            if !cfg.no_create {
                File::create_new(file)?;
            }
            Ok(())
        }
        Err(err) => error(err.to_string()),
    }
}
