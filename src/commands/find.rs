use std::fs;
use crate::stream::{OutputStream, InputStream};
use crate::data::{Cell, CellType, Row, Argument, ColumnType};
use crate::commands::{CompileContext, JobJoinHandle};
use crate::errors::{JobError, error, to_job_error, JobResult};
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
use crate::data::ArgumentVecCompiler;

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
    output: &mut OutputStream) -> JobResult<()> {
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
    output: &mut OutputStream) -> JobResult<()> {
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

pub fn run(mut config: Config) -> JobResult<()> {
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

pub fn compile_and_run_ls(context: CompileContext) -> JobResult<()> {
    let mut deps: Vec<JobJoinHandle> = Vec::new();
    let arguments = context.argument_definitions.compile(&mut deps, &context)?;
    let output = context.output.initialize(output_type.clone())?;
    let cfg = parse(output, arguments, false)?;
    run(cfg)
}

pub fn compile_and_run_find(context: CompileContext) -> JobResult<()> {
    let mut deps: Vec<JobJoinHandle> = Vec::new();
    let arguments = context.argument_definitions.compile(&mut deps, &context)?;
    let output = context.output.initialize(output_type.clone())?;
    let cfg = parse(output, arguments, true)?;
    run(cfg)
}
