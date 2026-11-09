use crate::data::table::ColumnFormat;
use crate::lang::argument::{ArgumentValue, ValidationArgument};
use crate::lang::ast::source::Source;
use crate::lang::command::OutputType::Unknown;
use crate::lang::command::{OutputType, PipeInputType};
use crate::lang::errors::{CrushResult, CrushResultExtra, data_error};
use crate::lang::pipe::TableOutputStream;
use crate::lang::signature::files;
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::global_state::GlobalState;
use crate::lang::{data::table::ColumnType, data::table::Row, value::Value, value::ValueType};
use crate::util::user_map::{create_group_map, create_user_map};
use chrono::{DateTime, Local};
use signature::signature;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::SystemTime;

enum Column {
    Permissions,
    Inode,
    Links,
    User,
    Group,
    Size,
    Blocks,
    Modified,
    Accessed,
    Type,
    File,
}

fn format_permissions(mode: u32) -> String {
    let mut res = String::with_capacity(9);
    let sticky = ((mode >> 9) & 1) != 0;
    let setgid = ((mode >> 9) & 2) != 0;
    let setuid = ((mode >> 9) & 4) != 0;
    for (sticky, set_owner, rwx) in vec![
        (false, setuid, (mode >> 6) & 7),
        (false, setgid, (mode >> 3) & 7),
        (sticky, false, mode & 7),
    ] {
        res.push(if (rwx & 4) != 0 { 'r' } else { '-' });
        res.push(if (rwx & 2) != 0 { 'w' } else { '-' });
        res.push(match (sticky, set_owner, (rwx & 1) != 0) {
            (false, false, false) => '-',
            (false, false, true) => 'x',
            (false, true, false) => 'S',
            (false, true, true) => 's',
            (true, false, false) => 'T',
            (true, false, true) => 't',
            _ => '?',
        });
    }
    res
}

fn insert_entity(
    meta: &Metadata,
    file: PathBuf,
    users: &HashMap<sysinfo::Uid, String>,
    groups: &HashMap<sysinfo::Gid, String>,
    cols: &[Column],
    output: &mut TableOutputStream,
) -> CrushResult<()> {
    let mut row = Vec::new();
    for col in cols.iter() {
        row.push(match col {
            Column::Permissions => {
                let permissions = format_permissions(meta.permissions().mode());
                Value::from(permissions)
            }
            Column::Inode => Value::from(meta.ino()),
            Column::Links => Value::from(meta.nlink()),
            Column::User => sysinfo::Uid::try_from(meta.uid() as usize)
                .ok()
                .and_then(|uid| users.get(&uid).map(|n| Value::from(n)))
                .unwrap_or_else(|| Value::from("?")),
            Column::Group => sysinfo::Gid::try_from(meta.gid() as usize)
                .ok()
                .and_then(|gid| groups.get(&gid).map(|n| Value::from(n)))
                .unwrap_or_else(|| Value::from("?")),
            Column::Size => Value::from(meta.len()),
            Column::Blocks => Value::from(meta.blocks()),
            Column::Modified => {
                let modified_system = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                let modified_datetime: DateTime<Local> = DateTime::from(modified_system);
                Value::Time(modified_datetime)
            }
            Column::Accessed => {
                let accessed_system = meta.accessed().unwrap_or(SystemTime::UNIX_EPOCH);
                let accessed_datetime: DateTime<Local> = DateTime::from(accessed_system);
                Value::Time(accessed_datetime)
            }
            Column::Type => {
                let file_type = meta.file_type();
                Value::from(if file_type.is_dir() {
                    "directory"
                } else if file_type.is_symlink() {
                    "symlink"
                } else {
                    "file"
                })
            }
            Column::File => Value::from(if file.starts_with("./") {
                let b = file.to_str().map(|s| PathBuf::from(&s[2..]));
                b.unwrap_or(file.clone())
            } else {
                file.clone()
            }),
        });
    }
    output.send(Row::new(row))
}

/// data_error(...) builds a bare CrushError with no command/source attached -- those
/// normally get filled in by the dispatch chain that evaluates a user's script
/// (Closure::eval()'s with_command()/with_source_fallback() calls), which these
/// internal per-entry errors never go through, since they're reported directly instead
/// of propagated. Attach them explicitly so a warning built from this is actually
/// useful: the reporting command's name and the $fs.files invocation's own location.
fn warn_data_error(global_state: &GlobalState, source: &Source, message: String) {
    global_state.warn(
        &data_error::<()>(message)
            .with_command("files")
            .with_source_fallback(source)
            .err()
            .unwrap(),
    );
}

fn run_for_single_directory_or_file(
    path: PathBuf,
    users: &HashMap<sysinfo::Uid, String>,
    groups: &HashMap<sysinfo::Gid, String>,
    recursive: bool,
    cols: &[Column],
    q: &mut VecDeque<PathBuf>,
    output: &mut TableOutputStream,
    global_state: &GlobalState,
    source: &Source,
) -> CrushResult<()> {
    if path.is_dir() {
        match fs::read_dir(&path) {
            Ok(dirs) => {
                for maybe_entry in dirs {
                    match maybe_entry {
                        Ok(entry) => {
                            match entry.metadata() {
                                Ok(meta) => {
                                    insert_entity(
                                        &meta,
                                        entry.path(),
                                        users,
                                        groups,
                                        cols,
                                        output,
                                    )?;
                                }
                                Err(err) => {
                                    warn_data_error(
                                        global_state,
                                        source,
                                        format!(
                                            "Failed to access metadata for file {}. Reason: {}",
                                            path.to_str().unwrap_or("<Illegal file name>"),
                                            err.to_string()
                                        ),
                                    );
                                }
                            }
                            if recursive
                                && entry.path().is_dir()
                                && (!(entry.file_name().eq(".") || entry.file_name().eq("..")))
                            {
                                q.push_back(entry.path());
                            }
                        }
                        Err(err) => {
                            warn_data_error(
                                global_state,
                                source,
                                format!(
                                    "Failed to list a file in directory {}. Reason: {}",
                                    path.to_str().unwrap_or("<Illegal file name>"),
                                    err.to_string()
                                ),
                            );
                        }
                    }
                }
            }
            Err(err) => {
                warn_data_error(
                    global_state,
                    source,
                    format!(
                        "Failed to list contents of directory {}. Reason: {}",
                        path.to_str().unwrap_or("<Illegal file name>"),
                        err.to_string()
                    ),
                );
            }
        }
    } else {
        match path.file_name() {
            Some(_) => match path.metadata() {
                Ok(p) => {
                    insert_entity(&p, path, users, groups, cols, output)?;
                }
                Err(err) => {
                    warn_data_error(
                        global_state,
                        source,
                        format!(
                            "Failed to access metadata for file {}. Reason: {}",
                            path.to_str().unwrap_or("<Illegal file name>"),
                            err.to_string()
                        ),
                    );
                }
            },
            None => {
                warn_data_error(
                    global_state,
                    source,
                    format!(
                        "Invalid file name {}.",
                        path.to_str().unwrap_or("<Illegal file name>")
                    ),
                );
            }
        }
    }
    Ok(())
}

#[signature(
    fs.files,
    short = "Show information about files and directories.",
    long = "If given no arguments, list the contents to the current working directory.",
    long = "If given any unnamed arguments, those will be the files and directories to list.",
    long = "",
    long = "By default, `files` will not recurse into subdirectories. You can override this using",
    long = "the `--recurse` switch.",
    output = Unknown,
    dynamic_output = files_dynamic_output)]
pub struct FilesSignature {
    #[unnamed()]
    #[description("directories and files to list")]
    directory: Vec<Files>,
    #[description("recurse into subdirectories")]
    #[default(false)]
    recurse: bool,
    #[description("show permissions")]
    #[default(true)]
    permissions: bool,
    #[description("show inode number")]
    #[default(false)]
    inode: bool,
    #[description("show link count")]
    #[default(true)]
    links: bool,
    #[description("show username")]
    #[default(true)]
    user: bool,
    #[description("show group name")]
    #[default(true)]
    group: bool,
    #[description("show file size")]
    #[default(true)]
    size: bool,
    #[description("show block count")]
    #[default(false)]
    blocks: bool,
    #[description("show modification time")]
    #[default(true)]
    modified: bool,
    #[description("show time of last file access")]
    #[default(false)]
    accessed: bool,
    #[description("show file type")]
    #[default(true)]
    r#type: bool,
    #[description("show file name")]
    #[default(true)]
    file: bool,
}

/// Whether each of `files`'s optional columns is included -- the single source of truth
/// for both `column_data` (building the real output from a fully parsed `FilesSignature`)
/// and `files_dynamic_output` (computing the same column list from raw, unparsed
/// arguments, before anything runs), so the two can never drift out of sync with each
/// other the way two separately hand-maintained column lists could.
struct ColumnFlags {
    permissions: bool,
    inode: bool,
    links: bool,
    user: bool,
    group: bool,
    size: bool,
    blocks: bool,
    modified: bool,
    accessed: bool,
    r#type: bool,
    file: bool,
}

impl Default for ColumnFlags {
    /// Mirrors every `#[default(...)]` on `FilesSignature`'s own boolean fields.
    fn default() -> Self {
        ColumnFlags {
            permissions: true,
            inode: false,
            links: true,
            user: true,
            group: true,
            size: true,
            blocks: false,
            modified: true,
            accessed: false,
            r#type: true,
            file: true,
        }
    }
}

impl From<&FilesSignature> for ColumnFlags {
    fn from(config: &FilesSignature) -> Self {
        ColumnFlags {
            permissions: config.permissions,
            inode: config.inode,
            links: config.links,
            user: config.user,
            group: config.group,
            size: config.size,
            blocks: config.blocks,
            modified: config.modified,
            accessed: config.accessed,
            r#type: config.r#type,
            file: config.file,
        }
    }
}

fn column_types(flags: &ColumnFlags) -> Vec<ColumnType> {
    let mut types = Vec::new();

    if flags.permissions {
        types.push(ColumnType::new("permissions", ValueType::String, "the file's permission bits, in symbolic form."));
    }
    if flags.inode {
        types.push(ColumnType::new("inode", ValueType::Integer, "the file's inode number."));
    }
    if flags.links {
        types.push(ColumnType::new("links", ValueType::Integer, "the number of hard links to the file."));
    }
    if flags.user {
        types.push(ColumnType::new("user", ValueType::String, "the name of the file's owning user."));
    }
    if flags.group {
        types.push(ColumnType::new("group", ValueType::String, "the name of the file's owning group."));
    }
    if flags.size {
        types.push(ColumnType::new_with_format(
            "size",
            ColumnFormat::ByteUnit,
            ValueType::Integer,
            "the file's size in bytes.",
        ));
    }
    if flags.blocks {
        types.push(ColumnType::new("blocks", ValueType::Integer, "the number of blocks allocated to the file."));
    }
    if flags.modified {
        types.push(ColumnType::new("modified", ValueType::Time, "when the file's contents were last modified."));
    }
    if flags.accessed {
        types.push(ColumnType::new("accessed", ValueType::Time, "when the file was last accessed."));
    }
    if flags.r#type {
        types.push(ColumnType::new("type", ValueType::String, "the kind of file, e.g. file, directory, or symlink."));
    }
    if flags.file {
        types.push(ColumnType::new("file", ValueType::File, "the file itself."));
    }

    types
}

fn column_data(config: &FilesSignature) -> (Vec<ColumnType>, Vec<Column>) {
    let flags = ColumnFlags::from(config);
    let types = column_types(&flags);
    let mut cols = Vec::new();

    if flags.permissions {
        cols.push(Column::Permissions);
    }
    if flags.inode {
        cols.push(Column::Inode);
    }
    if flags.links {
        cols.push(Column::Links);
    }
    if flags.user {
        cols.push(Column::User);
    }
    if flags.group {
        cols.push(Column::Group);
    }
    if flags.size {
        cols.push(Column::Size);
    }
    if flags.blocks {
        cols.push(Column::Blocks);
    }
    if flags.modified {
        cols.push(Column::Modified);
    }
    if flags.accessed {
        cols.push(Column::Accessed);
    }
    if flags.r#type {
        cols.push(Column::Type);
    }
    if flags.file {
        cols.push(Column::File);
    }

    (types, cols)
}

/// Computes `files`'s real output type: a `table_input_stream` whose columns are exactly
/// the ones that will actually be shown, which depends on every one of its boolean
/// arguments (`permissions=`, `size=`, ...) -- each defaulting the same way
/// `FilesSignature`'s own fields do when not given at all. `directory`/`recurse` don't
/// affect which columns are shown, so they're simply ignored here.
fn files_dynamic_output(
    _input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    let mut flags = ColumnFlags::default();
    for arg in arguments {
        let Some(name) = &arg.argument_type else {
            continue;
        };
        let target: &mut bool = match name.as_str() {
            "permissions" => &mut flags.permissions,
            "inode" => &mut flags.inode,
            "links" => &mut flags.links,
            "user" => &mut flags.user,
            "group" => &mut flags.group,
            "size" => &mut flags.size,
            "blocks" => &mut flags.blocks,
            "modified" => &mut flags.modified,
            "accessed" => &mut flags.accessed,
            "type" => &mut flags.r#type,
            "file" => &mut flags.file,
            _ => continue,
        };
        match &arg.value {
            ArgumentValue::Value(Value::Bool(b)) => *target = *b,
            ArgumentValue::Unknown | ArgumentValue::Type(_) => return Ok(OutputType::Unknown),
            ArgumentValue::Value(_) => {
                return arg.error(format!("`{}` must be given a boolean value", name));
            }
        }
    }
    Ok(OutputType::Known(ValueType::TableInputStream(column_types(
        &flags,
    ))))
}

fn files(mut context: CommandContext) -> CrushResult<()> {
    let config: FilesSignature =
        FilesSignature::parse(context.remove_arguments(), &context.global_state.printer())?;

    let (types, cols) = column_data(&config);

    let mut output = context.initialize_output(&types)?;

    let mut dir = if !config.directory.is_empty() {
        files::into_paths(config.directory)?
    } else {
        vec![PathBuf::from(".")]
    };
    let users = create_user_map()?;
    let groups = create_group_map()?;
    let mut q = VecDeque::new();
    q.extend(dir.drain(..));
    loop {
        match q.pop_front() {
            None => break,
            Some(dir) => run_for_single_directory_or_file(
                dir,
                &users,
                &groups,
                config.recurse,
                &cols,
                &mut q,
                &mut output,
                &context.global_state,
                &context.source,
            )?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::argument::SwitchStyle;
    use crate::lang::ast::source::{Source, SourceType};
    use std::sync::Arc;

    fn bool_arg(name: &str, value: ArgumentValue) -> ValidationArgument {
        ValidationArgument {
            argument_type: Some(name.to_string()),
            switch_style: SwitchStyle::None,
            value,
            source: Source::new(SourceType::Input, Arc::from("")),
        }
    }

    fn column_names(args: &[ValidationArgument]) -> Vec<String> {
        match files_dynamic_output(PipeInputType::None, args).unwrap() {
            OutputType::Known(ValueType::TableInputStream(columns)) => {
                columns.iter().map(|c| c.name().to_string()).collect()
            }
            other => panic!("expected Known(TableInputStream), got {:?}", other),
        }
    }

    #[test]
    fn default_columns_match_the_signature_defaults() {
        let names = column_names(&[]);
        assert_eq!(
            names,
            vec!["permissions", "links", "user", "group", "size", "modified", "type", "file"]
        );
    }

    #[test]
    fn disabling_a_default_column_removes_it() {
        let names = column_names(&[bool_arg("user", ArgumentValue::Value(Value::Bool(false)))]);
        assert!(!names.contains(&"user".to_string()));
    }

    #[test]
    fn enabling_a_non_default_column_adds_it() {
        let names = column_names(&[bool_arg("inode", ArgumentValue::Value(Value::Bool(true)))]);
        assert!(names.contains(&"inode".to_string()));
    }

    #[test]
    fn unrelated_arguments_are_ignored() {
        let names = column_names(&[bool_arg("recurse", ArgumentValue::Value(Value::Bool(true)))]);
        assert_eq!(
            names,
            vec!["permissions", "links", "user", "group", "size", "modified", "type", "file"]
        );
    }

    #[test]
    fn non_boolean_column_value_is_an_error() {
        let args = [bool_arg("size", ArgumentValue::Value(Value::from("oops")))];
        assert!(files_dynamic_output(PipeInputType::None, &args).is_err());
    }

    #[test]
    fn unresolved_column_value_falls_back_to_unknown() {
        let args = [bool_arg("size", ArgumentValue::Type(ValueType::Bool))];
        match files_dynamic_output(PipeInputType::None, &args).unwrap() {
            OutputType::Unknown => {}
            other => panic!("expected Unknown, got {:?}", other),
        }
    }
}
