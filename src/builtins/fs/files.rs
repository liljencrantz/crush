use std::collections::{HashMap, VecDeque};
use std::fs;
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use chrono::{DateTime, Local};
use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::{error, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::signature::files::Files;
use crate::lang::pipe::OutputStream;
use crate::lang::{data::table::ColumnType, data::table::Row, value::Value, value::ValueType};
use crate::util::user_map::{create_user_map, create_group_map};
use signature::signature;
use std::os::unix::fs::PermissionsExt;
use nix::unistd::{Uid, Gid};
use crate::data::table::ColumnFormat;

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
    for (sticky, set_owner, rwx) in vec![(false, setuid, (mode >> 6) & 7), (false, setgid, (mode >> 3) & 7), (sticky, false, mode & 7)] {
        res.push(if (rwx & 4) != 0 { 'r' } else { '-' });
        res.push(if (rwx & 2) != 0 { 'w' } else { '-' });
        res.push(
            match (sticky, set_owner, (rwx & 1) != 0) {
                (false, false, false) => '-',
                (false, false, true) => 'x',
                (false, true, false) => 'S',
                (false, true, true) => 's',
                (true, false, false) => 'T',
                (true, false, true) => 't',
                _ => '?',
            }
        );
    }
    res
}

fn insert_entity(
    meta: &Metadata,
    file: PathBuf,
    users: &HashMap<Uid, String>,
    groups: &HashMap<Gid, String>,
    cols: &[Column],
    output: &mut OutputStream,
) -> CrushResult<()> {
    let mut row = Vec::new();
    for col in cols.iter() {
        row.push(match col {
            Column::Permissions => {
                let permissions = format_permissions(meta.permissions().mode());
                Value::from(permissions)
            }
            Column::Inode => Value::Integer(i128::from(meta.ino())),
            Column::Links => Value::Integer(i128::from(meta.nlink())),
            Column::User => users.get(&Uid::from_raw(meta.uid())).map(|n| Value::from(n)).unwrap_or_else(|| Value::from("?")),
            Column::Group => groups.get(&Gid::from_raw(meta.gid())).map(|n| Value::from(n)).unwrap_or_else(|| Value::from("?")),
            Column::Size => Value::Integer(i128::from(meta.len())),
            Column::Blocks => Value::Integer(i128::from(meta.blocks())),
            Column::Modified => {
                let modified_system = meta.modified()?;
                let modified_datetime: DateTime<Local> = DateTime::from(modified_system);
                Value::Time(modified_datetime)
            }
            Column::Accessed => {
                let accessed_system = meta.accessed()?;
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
            },
            Column::File =>
                Value::from(if file.starts_with("./") {
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
    users: &HashMap<Uid, String>,
    groups: &HashMap<Gid, String>,
    recursive: bool,
    cols: &[Column],
    q: &mut VecDeque<PathBuf>,
    output: &mut OutputStream,
) -> CrushResult<()> {
    if path.is_dir() {
        let dirs = fs::read_dir(path);
        for maybe_entry in dirs? {
            let entry = maybe_entry?;
            insert_entity(
                &entry.metadata()?,
                entry.path(),
                users,
                groups,
                cols,
                output,
            )?;
            if recursive
                && entry.path().is_dir()
                && (!(entry.file_name().eq(".") || entry.file_name().eq("..")))
            {
                q.push_back(entry.path());
            }
        }
    } else {
        match path.file_name() {
            Some(_) => {
                insert_entity(&path.metadata()?, path, users, groups, cols, output)?;
            }
            None => {
                return error("Invalid file name");
            }
        }
    }
    Ok(())
}

#[signature(fs.files, short = "Recursively list files", output = Unknown)]
pub struct FilesSignature {
    #[unnamed()]
    #[description("directories and files to list")]
    directory: Files,
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
        types.push(ColumnType::new_with_format("size", ColumnFormat::ByteUnit, ValueType::Integer));
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

fn files(context: CommandContext) -> CrushResult<()> {
    let config: FilesSignature = FilesSignature::parse(context.arguments, &context.global_state.printer())?;

    let (types, cols) = column_data(&config);

    let mut output = context.output.initialize(&types)?;

    let mut dir = if config.directory.had_entries() {
        Vec::from(config.directory)
    } else {
        vec![PathBuf::from(".")]
    };
    let users = create_user_map()?;
    let groups = create_group_map()?;
    let mut q = VecDeque::new();
    q.extend(dir.drain(..));
    loop {
        if q.is_empty() {
            break;
        }
        let dir = q.pop_front().unwrap();
        let _ =
            run_for_single_directory_or_file(dir, &users, &groups, config.recurse, &cols, &mut q, &mut output);
    }
    Ok(())
}
