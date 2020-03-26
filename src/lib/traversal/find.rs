use std::collections::{HashMap, VecDeque};
use std::fs;
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use chrono::{DateTime, Local};
use users::uid_t;
use users::User;

use lazy_static::lazy_static;

use crate::lang::command::ExecutionContext;
use crate::lib::command_util::{create_user_map, UserMap};
use crate::lang::{argument::Argument, value::Value, value::ValueType, table::ColumnType, table::Row};
use crate::util::file::cwd;
use crate::lang::errors::{error, CrushError, CrushResult, to_crush_error};
use crate::lang::stream::OutputStream;

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
    file: Box<Path>,
    users: &HashMap<uid_t, User>,
    output: &mut OutputStream) -> CrushResult<()> {
    let modified_system = to_crush_error(meta.modified())?;
    let modified_datetime: DateTime<Local> = DateTime::from(modified_system);
    let f = if file.starts_with("./") {
        let b = file.to_str().map(|s| Box::from(Path::new(&s[2..])));
        b.unwrap_or(file)
    } else {
        file
    };
    let file_type = meta.file_type();
    let ftype = if file_type.is_dir() {
        "directory"
    } else {
        if file_type.is_symlink() {
            "symlink"
        } else {
            "file"
        }
    };

    output.send(Row ::new(vec![
        users.get_name(meta.uid()),
        Value::Integer(i128::from(meta.len())),
        Value::Time(modified_datetime),
        Value::string(ftype),
        Value::File(f)]))?;
    Ok(())
}

fn run_for_single_directory_or_file(
    path: Box<Path>,
    users: &HashMap<uid_t, User>,
    recursive: bool,
    q: &mut VecDeque<Box<Path>>,
    output: &mut OutputStream) -> CrushResult<()> {
    if path.is_dir() {
        let dirs = fs::read_dir(path);
        for maybe_entry in to_crush_error(dirs)? {
            let entry = to_crush_error(maybe_entry)?;
            insert_entity(
                &to_crush_error(entry.metadata())?,
                entry.path().into_boxed_path(),
                &users,
                output)?;
            if recursive && entry.path().is_dir() {
                if !(entry.file_name().eq(".") || entry.file_name().eq("..")) {
                    q.push_back(entry.path().into_boxed_path());
                }
            }
        }
    } else {
        match path.file_name() {
            Some(_) => {
                insert_entity(
                    &to_crush_error(path.metadata())?,
                    path,
                    &users,
                    output)?;
            }
            None => {
                return error("Invalid file name");
            }
        }
    }
    Ok(())
}

pub fn run(mut config: Config) -> CrushResult<()> {
    let users = create_user_map();
    let mut q = VecDeque::new();
    for dir in config.dirs {
        q.push_back(dir);
    }
    loop {
        if q.is_empty() {
            break;
        }
        let dir = q.pop_front().unwrap();
        run_for_single_directory_or_file(dir, &users, config.recursive, &mut q, &mut config.output);
    }
    return Ok(());
}

pub struct Config {
    dirs: Vec<Box<Path>>,
    recursive: bool,
    output: OutputStream,
}

fn parse(output: OutputStream, arguments: Vec<Argument>, recursive: bool) -> Result<Config, CrushError> {
    let mut dirs: Vec<Box<Path>> = Vec::new();
    if arguments.is_empty() {
        dirs.push(Box::from(Path::new(".")));
    }
    for arg in arguments {
        match &arg.value {
            Value::String(dir) =>
                dirs.push(Box::from(Path::new(dir.as_ref()))),
            Value::File(dir) =>
                dirs.push(dir.clone()),
            Value::Glob(dir) => dir.glob_files(&cwd()?, &mut dirs)?,
            _ => {
                return error("Invalid argument type to ls, expected string or glob");
            }
        }
    }
    Ok(Config { dirs, recursive, output })
}

pub fn perform_ls(context: ExecutionContext) -> CrushResult<()> {
    let output = context.output.initialize(OUTPUT_TYPE.clone())?;
    let cfg = parse(output, context.arguments, false)?;
    run(cfg)
}

pub fn perform_find(context: ExecutionContext) -> CrushResult<()> {
    let output = context.output.initialize(OUTPUT_TYPE.clone())?;
    let cfg = parse(output, context.arguments, true)?;
    run(cfg)
}
