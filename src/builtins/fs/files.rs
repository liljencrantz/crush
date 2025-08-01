use crate::data::table::ColumnFormat;
use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::{CrushResult, data_error};
use crate::lang::pipe::TableOutputStream;
use crate::lang::printer::Printer;
use crate::lang::signature::binary_input::BinaryInput;
use crate::lang::signature::files;
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
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
use std::sync::Arc;
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

fn run_for_single_directory_or_file(
    path: PathBuf,
    users: &HashMap<sysinfo::Uid, String>,
    groups: &HashMap<sysinfo::Gid, String>,
    recursive: bool,
    cols: &[Column],
    q: &mut VecDeque<PathBuf>,
    output: &mut TableOutputStream,
    printer: &Printer,
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
                                    printer.crush_error(
                                        data_error::<()>(format!(
                                            "Failed to access metadata for file {}. Reason: {}",
                                            path.to_str().unwrap_or("<Illegal file name>"),
                                            err.to_string()
                                        ))
                                        .err()
                                        .unwrap(),
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
                            printer.crush_error(
                                data_error::<()>(format!(
                                    "Failed to list a file in directory {}. Reason: {}",
                                    path.to_str().unwrap_or("<Illegal file name>"),
                                    err.to_string()
                                ))
                                .err()
                                .unwrap(),
                            );
                        }
                    }
                }
            }
            Err(err) => {
                printer.crush_error(
                    data_error::<()>(format!(
                        "Failed to list contents of directory {}. Reason: {}",
                        path.to_str().unwrap_or("<Illegal file name>"),
                        err.to_string()
                    ))
                    .err()
                    .unwrap(),
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
                    printer.crush_error(
                        data_error::<()>(format!(
                            "Failed to access metadata for file {}. Reason: {}",
                            path.to_str().unwrap_or("<Illegal file name>"),
                            err.to_string()
                        ))
                        .err()
                        .unwrap(),
                    );
                }
            },
            None => {
                printer.crush_error(
                    data_error::<()>(format!(
                        "Invalid file name {}.",
                        path.to_str().unwrap_or("<Illegal file name>")
                    ))
                    .err()
                    .unwrap(),
                );
            }
        }
    }
    Ok(())
}

#[signature(
    fs.files,
    short = "Show information about files and directories",
    long = "If given no arguments, list the contents to the current working directory.",
    long = "If given any unnamed arguments, those will be the files and directories to list.",
    long = "",
    long = "By default, `files` will not recurse into subdirectories. You can override this using",
    long = "the `--recurse` switch.",
    output = Unknown)]
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

fn column_data(config: &FilesSignature) -> (Vec<ColumnType>, Vec<Column>) {
    let mut types = Vec::new();
    let mut cols = Vec::new();

    if config.permissions {
        types.push(ColumnType::new("permissions", ValueType::String));
        cols.push(Column::Permissions);
    }
    if config.inode {
        types.push(ColumnType::new("inode", ValueType::Integer));
        cols.push(Column::Inode);
    }
    if config.links {
        types.push(ColumnType::new("links", ValueType::Integer));
        cols.push(Column::Links);
    }
    if config.user {
        types.push(ColumnType::new("user", ValueType::String));
        cols.push(Column::User);
    }
    if config.group {
        types.push(ColumnType::new("group", ValueType::String));
        cols.push(Column::Group);
    }
    if config.size {
        types.push(ColumnType::new_with_format(
            "size",
            ColumnFormat::ByteUnit,
            ValueType::Integer,
        ));
        cols.push(Column::Size);
    }
    if config.blocks {
        types.push(ColumnType::new("blocks", ValueType::Integer));
        cols.push(Column::Blocks);
    }
    if config.modified {
        types.push(ColumnType::new("modified", ValueType::Time));
        cols.push(Column::Modified);
    }
    if config.accessed {
        types.push(ColumnType::new("accessed", ValueType::Time));
        cols.push(Column::Accessed);
    }
    if config.r#type {
        types.push(ColumnType::new("type", ValueType::String));
        cols.push(Column::Type);
    }
    if config.file {
        types.push(ColumnType::new("file", ValueType::File));
        cols.push(Column::File);
    }

    (types, cols)
}

fn files(mut context: CommandContext) -> CrushResult<()> {
    let config: FilesSignature =
        FilesSignature::parse(context.remove_arguments(), &context.global_state.printer())?;

    let (types, cols) = column_data(&config);

    let mut output = context.output.initialize(&types)?;

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
                &context.global_state.printer(),
            )?,
        }
    }
    Ok(())
}
