use std::fs;
use crate::stream::{OutputStream, InputStream};
use crate::data::{Cell, CellType, Row, Argument, ColumnType};
use crate::commands::Exec;
use crate::errors::{JobError, error, to_job_error};
use chrono::{Local, DateTime};
use std::path::Path;
use std::fs::Metadata;
use crate::env::{get_cwd, Env};
use crate::printer::Printer;
use crate::commands::command_util::{create_user_map, UserMap};
use std::collections::{HashMap, VecDeque};
use users::uid_t;
use users::User;
use std::os::unix::fs::MetadataExt;
use lazy_static::lazy_static;

lazy_static! {
    static ref output_type: Vec<ColumnType> = vec![
        ColumnType::named("user", CellType::Text),
        ColumnType::named("size", CellType::Integer),
        ColumnType::named("modified", CellType::Time),
        ColumnType::named("file", CellType::Text),
    ];
}

fn insert_entity(
    meta: &Metadata,
    file: Box<Path>,
    users: &HashMap<uid_t, User>,
    output: &mut OutputStream) -> Result<(), JobError> {
    let modified_system = to_job_error(meta.modified())?;
    let modified_datetime: DateTime<Local> = DateTime::from(modified_system);
    let f = if file.starts_with("./") {
        let b = file.to_str().map(|s| Box::from(Path::new(&s[2..])));
        b.unwrap_or(file)
    } else {
        file
    };
    output.send(Row {
        cells: vec![
            users.get_name(meta.uid()),
            Cell::Integer(i128::from(meta.len())),
            Cell::Time(modified_datetime),
            Cell::File(f),
        ]
    })?;
    return Ok(());
}

fn run_for_single_directory_or_file(
    path: Box<Path>,
    users: &HashMap<uid_t, User>,
    recursive: bool,
    q: &mut VecDeque<Box<Path>>,
    output: &mut OutputStream) -> Result<(), JobError> {
    if path.is_dir() {
        let dirs = fs::read_dir(path);
        for maybe_entry in to_job_error(dirs)? {
            let entry = to_job_error(maybe_entry)?;
            insert_entity(
                &to_job_error(entry.metadata())?,
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
                    &to_job_error(path.metadata())?,
                    path,
                    &users,
                    output)?;
            }
            None => {
                return Err(error("Invalid file name"));
            }
        }
    }
    Ok(())
}

pub fn run(mut config: Config, env: Env, printer: Printer) -> Result<(), JobError> {
    let users = create_user_map();
    let mut q = VecDeque::new();
        for dir in config.dirs {
            q.push_back(dir);
        }
    loop {
        if (q.is_empty()) {
            break;
        }
        let dir = q.pop_front().unwrap();
        run_for_single_directory_or_file(dir, &users, config.recursive, &mut q, &mut config.output);
    }
    return Ok(());
}

pub fn compile_ls(input_type: Vec<ColumnType>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<ColumnType>), JobError> {
    let cfg = parse(output, arguments, false)?;
    Ok((Exec::Find(cfg), output_type.clone()))
}

pub struct Config {
    dirs: Vec<Box<Path>>,
    recursive: bool,
    output: OutputStream,
}

fn parse(output: OutputStream, arguments: Vec<Argument>, recursive: bool) -> Result<Config, JobError> {
    let mut dirs: Vec<Box<Path>> = Vec::new();
    if arguments.is_empty() {
        dirs.push(Box::from(Path::new(".")));
    }
    for arg in arguments {
        match &arg.cell {
            Cell::Text(dir) => {
                dirs.push(Box::from(Path::new(dir.as_ref())));
            }
            Cell::File(dir) => {
                dirs.push(dir.clone());
            }
            Cell::Glob(dir) => {
                to_job_error(
                    dir.glob_files(
                        &get_cwd()?,
                        &mut dirs))?;
            }
            _ => {
                return Err(error("Invalid argument type to ls, expected string or glob"));
            }
        }
    }
    Ok(Config { dirs, recursive, output })
}

pub fn compile_find(input_type: Vec<ColumnType>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<ColumnType>), JobError> {
    let cfg = parse(output, arguments, true)?;
    Ok((Exec::Find(cfg), output_type.clone()))
}
