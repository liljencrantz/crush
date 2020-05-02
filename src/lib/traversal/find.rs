use std::collections::{HashMap, VecDeque};
use std::fs;
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use chrono::{DateTime, Local};
use users::uid_t;
use users::User;

use lazy_static::lazy_static;

use crate::lang::argument::ArgumentHandler;
use crate::lang::errors::{error, to_crush_error, CrushResult};
use crate::lang::execution_context::ExecutionContext;
use crate::lang::stream::OutputStream;
use crate::lang::{table::ColumnType, table::Row, value::Value, value::ValueType};
use crate::util::user_map::{create_user_map, UserMap};
use signature::signature;

lazy_static! {
    static ref OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("user", ValueType::String),
        ColumnType::new("size", ValueType::Integer),
        ColumnType::new("modified", ValueType::Time),
        ColumnType::new("type", ValueType::String),
        ColumnType::new("file", ValueType::File),
    ];
}

fn insert_entity(
    meta: &Metadata,
    file: PathBuf,
    users: &HashMap<uid_t, User>,
    output: &mut OutputStream,
) -> CrushResult<()> {
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
        users.get_name(meta.uid()),
        Value::Integer(i128::from(meta.len())),
        Value::Time(modified_datetime),
        Value::string(type_str),
        Value::File(f),
    ]))?;
    Ok(())
}

fn run_for_single_directory_or_file(
    path: PathBuf,
    users: &HashMap<uid_t, User>,
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
                &users,
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
                insert_entity(&to_crush_error(path.metadata())?, path, &users, output)?;
            }
            None => {
                return error("Invalid file name");
            }
        }
    }
    Ok(())
}

#[signature]
#[derive(Debug)]
struct Signature {
    #[unnamed()]
    dirs: Vec<PathBuf>,
    recursive: bool,
}

pub fn find(context: ExecutionContext) -> CrushResult<()> {
    let mut output = context.output.initialize(OUTPUT_TYPE.clone())?;
    let mut config: Signature = Signature::parse(context.arguments, &context.printer)?;

    if config.dirs.len() == 0 {
        config.dirs.push(PathBuf::from("."));
    }
    let users = create_user_map();
    let mut q = VecDeque::new();
    q.extend(config.dirs.drain(..));
    loop {
        if q.is_empty() {
            break;
        }
        let dir = q.pop_front().unwrap();
        let _ =
            run_for_single_directory_or_file(dir, &users, config.recursive, &mut q, &mut output);
    }
    Ok(())
}
