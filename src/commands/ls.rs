use std::{io, fs};
use crate::stream::{OutputStream, InputStream};
use crate::cell::{Argument, CellType, Cell, Row, CellDataType};
use crate::commands::{Call, to_runtime_error};
use crate::errors::JobError;
use chrono::{Local, DateTime};
use crate::state::State;
use crate::glob::glob_files;
use std::path::Path;
use std::fs::Metadata;
use std::ffi::OsStr;

fn insert_entity(meta: &Metadata, file_name: &OsStr, output: &mut OutputStream) -> io::Result<()> {
    let modified_system = meta.modified()?;
    let modified_datetime: DateTime<Local> = DateTime::from(modified_system);
    match file_name.to_str() {
        Some(name) =>
            output.add(Row {
                cells: vec![
                    Cell::Text(String::from(name)),
                    Cell::Integer(i128::from(meta.len())),
                    Cell::Time(modified_datetime),
                ]
            }),
        None => {
            return Err(io::Error::new(io::ErrorKind::Other, "Invalid file name"));
        }
    }
    return Ok(());
}

fn run_for_single_directory_or_file(
    file: &str,
    output: &mut OutputStream) -> Result<(), io::Error> {
    let path = Path::new(file);
    if path.is_dir() {
        let dirs = fs::read_dir(path);
        for maybe_entry in dirs? {
            let entry = maybe_entry?;
            insert_entity(
                &entry.metadata()?,
                entry.file_name().as_os_str(),
                output)?;
        }
    } else {
        match path.file_name() {
            Some(name) => {
                insert_entity(
                    &path.metadata()?,
                    name,
                    output)?;
            }
            None => {
                return Err(io::Error::new(io::ErrorKind::Other, "Invalid file name"));
            }
        }
    }
    Ok(())
}

fn run_internal(
    arguments: &Vec<Argument>,
    output: &mut OutputStream) -> Result<(), io::Error> {
    let mut dirs: Vec<String> = Vec::new();
    if arguments.is_empty() {
        dirs.push(String::from("."));
    } else {
        for arg in arguments {
            match &arg.cell {
                Cell::Text(dir) => {
                    dirs.push(dir.clone());
                }
                Cell::Glob(dir) => {
                    glob_files(dir, Path::new(std::env::current_dir()?.to_str().expect("Invalid directory name")), &mut dirs)?;
                }
                _ => {
                    return Err(io::Error::new(io::ErrorKind::Other, "Invalid argument type to ls, expected string or glob"));
                }
            }
        }
    }

    for dir in dirs {
        run_for_single_directory_or_file(
            dir.as_str(), output)?;
    }
    return Ok(());
}

fn run(
    _input_type: &Vec<CellType>,
    arguments: &Vec<Argument>,
    _input: &mut InputStream,
    output: &mut OutputStream) -> Result<(), JobError> {
    return to_runtime_error(run_internal(arguments, output));
}

pub fn ls(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Call, JobError> {
    return Ok(Call {
        name: String::from("ls"),
        input_type: input_type.clone(),
        arguments: arguments.clone(),
        output_type: vec![
            CellType {
                name: String::from("file"),
                cell_type: CellDataType::Text,
            },
            CellType {
                name: String::from("size"),
                cell_type: CellDataType::Integer,
            },
            CellType {
                name: String::from("modified"),
                cell_type: CellDataType::Time,
            },
        ],
        run_internal: run,
        mutate_internal: |_1, _2, _3| { Ok(()) },
    });
}
