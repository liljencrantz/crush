use std::collections::{HashMap, VecDeque};
use std::fs;
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use chrono::{DateTime, Local};

use lazy_static::lazy_static;

use crate::lang::command::OutputType::Known;
use crate::lang::errors::{error, to_crush_error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::files::Files;
use crate::lang::pipe::OutputStream;
use crate::lang::{data::table::ColumnType, data::table::Row, value::Value, value::ValueType};
use crate::util::user_map::{create_user_map, create_group_map};
use signature::signature;
use std::os::unix::fs::PermissionsExt;
use nix::unistd::{Uid, Gid};

lazy_static! {
    static ref OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("permissions", ValueType::String),
        ColumnType::new("user", ValueType::String),
        ColumnType::new("group", ValueType::String),
        ColumnType::new("size", ValueType::Integer),
        ColumnType::new("modified", ValueType::Time),
        ColumnType::new("type", ValueType::String),
        ColumnType::new("file", ValueType::File),
    ];
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
    output: &mut OutputStream,
) -> CrushResult<()> {
    let mode = meta.permissions().mode();
    let permissions = format_permissions(mode);
    let modified_system = to_crush_error(meta.modified())?;
    let modified_datetime: DateTime<Local> = DateTime::from(modified_system);
    let f = if file.starts_with("./") {
        let b = file.to_str().map(|s| PathBuf::from(&s[2..]));
        b.unwrap_or(file)
    } else {
        file
    };
    let file_type = meta.file_type();
    let type_str = if file_type.is_dir() {
        "directory"
    } else if file_type.is_symlink() {
        "symlink"
    } else {
        "file"
    };

    output.send(Row::new(vec![
        Value::String(permissions),
        users.get(&Uid::from_raw(meta.uid())).map(|n| Value::string(n)).unwrap_or_else(|| Value::string("?")),
        groups.get(&Gid::from_raw(meta.gid())).map(|n| Value::string(n)).unwrap_or_else(|| Value::string("?")),
        Value::Integer(i128::from(meta.len())),
        Value::Time(modified_datetime),
        Value::string(type_str),
        Value::File(f),
    ]))?;
    Ok(())
}

fn run_for_single_directory_or_file(
    path: PathBuf,
    users: &HashMap<Uid, String>,
    groups: &HashMap<Gid, String>,
    recursive: bool,
    q: &mut VecDeque<PathBuf>,
    output: &mut OutputStream,
) -> CrushResult<()> {
    if path.is_dir() {
        let dirs = fs::read_dir(path);
        for maybe_entry in to_crush_error(dirs)? {
            let entry = to_crush_error(maybe_entry)?;
            insert_entity(
                &to_crush_error(entry.metadata())?,
                entry.path(),
                users,
                groups,
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
                insert_entity(&to_crush_error(path.metadata())?, path, users, groups, output)?;
            }
            None => {
                return error("Invalid file name");
            }
        }
    }
    Ok(())
}

#[signature(find, short = "Recursively list files", output = Known(ValueType::TableInputStream(OUTPUT_TYPE.clone())))]
pub struct Find {
    #[unnamed()]
    #[description("directories and files to list")]
    directory: Files,
    #[description("recurse into subdirectories")]
    #[default(true)]
    recursive: bool,
}

fn find(context: CommandContext) -> CrushResult<()> {
    let mut output = context.output.initialize(OUTPUT_TYPE.clone())?;
    let config: Find = Find::parse(context.arguments, &context.global_state.printer())?;

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
            run_for_single_directory_or_file(dir, &users, &groups, config.recursive, &mut q, &mut output);
    }
    Ok(())
}
