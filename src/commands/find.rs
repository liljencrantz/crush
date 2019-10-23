use std::fs;
use crate::stream::{OutputStream, InputStream};
use crate::data::{Cell, CellType, Row, Argument, CellFnurp};
use crate::commands::{Exec};
use crate::errors::{JobError, error, to_job_error};
use chrono::{Local, DateTime};
use std::path::Path;
use std::fs::Metadata;
use crate::env::{get_cwd, Env};
use crate::printer::Printer;

fn insert_entity(meta: &Metadata, file: Box<Path>, output: &mut OutputStream) -> Result<(), JobError> {
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
            Cell::File(f),
            Cell::Integer(i128::from(meta.len())),
            Cell::Time(modified_datetime),
        ]
    })?;
    return Ok(());
}

fn run_for_single_directory_or_file(
    path: Box<Path>,
    recursive: bool,
    output: &mut OutputStream) -> Result<(), JobError> {
    if path.is_dir() {
        let dirs = fs::read_dir(path);
        for maybe_entry in to_job_error(dirs)? {
            let entry = to_job_error(maybe_entry)?;
            insert_entity(
                &to_job_error(entry.metadata())?,
                entry.path().into_boxed_path(),
                output)?;
            if recursive && entry.path().is_dir() {
                if !(entry.file_name().eq(".") || entry.file_name().eq("..")) {
                    run_for_single_directory_or_file(
                        entry.path().into_boxed_path(),
                        true,
                        output)?;
                }
            }
        }
    } else {
        match path.file_name() {
            Some(_) => {
                insert_entity(
                    &to_job_error(path.metadata())?,
                    path,
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
    for dir in config.dirs {
        run_for_single_directory_or_file(dir, config.recursive, &mut config.output);
    }
    return Ok(());
}

pub fn compile_ls(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    let cfg = parse(output, arguments, false)?;
    Ok((Exec::Find(cfg), vec![
        CellFnurp::named("file", CellType::Text),
        CellFnurp::named("size", CellType::Integer),
        CellFnurp::named("modified", CellType::Time),
    ]))
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
    Ok(Config{ dirs, recursive, output })
}

pub fn compile_find(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    let cfg = parse(output, arguments, true)?;
    Ok((Exec::Find(cfg), vec![
        CellFnurp::named("file", CellType::Text),
        CellFnurp::named("size", CellType::Integer),
        CellFnurp::named("modified", CellType::Time),
    ]))
}
