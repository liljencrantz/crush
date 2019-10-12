use std::{io, fs};
use crate::stream::{OutputStream, InputStream};
use crate::cell::{Argument, CellType, Cell, Row, CellDataType};
use crate::commands::{Call, to_runtime_error};
use crate::errors::{JobError, error};
use chrono::{Local, DateTime};
use crate::glob::glob_files;
use std::path::Path;
use std::fs::Metadata;
use std::ffi::OsStr;

fn insert_entity(meta: &Metadata, file: Box<Path>, output: &mut OutputStream) -> io::Result<()> {
    let modified_system = meta.modified()?;
    let modified_datetime: DateTime<Local> = DateTime::from(modified_system);
            output.send(Row {
                cells: vec![
                    Cell::File(file),
                    Cell::Integer(i128::from(meta.len())),
                    Cell::Time(modified_datetime),
                ]
            });
    return Ok(());
}

fn run_for_single_directory_or_file(
    file: Box<Path>,
    recursive: bool,
    output: &mut OutputStream) -> Result<(), io::Error> {
    let path = file;
    if path.is_dir() {
        let dirs = fs::read_dir(path);
        for maybe_entry in dirs? {
            let entry = maybe_entry?;
            insert_entity(
                &entry.metadata()?,
                entry.path().into_boxed_path(),
                output)?;
            if recursive && entry.path().is_dir() {
                if !(entry.file_name().eq(".") || entry.file_name().eq("..")) {
                    run_for_single_directory_or_file(
                        entry.path().into_boxed_path(),
                        true,
                        output);
                }
            }
        }
    } else {
        match path.file_name() {
            Some(name) => {
                insert_entity(
                    &path.metadata()?,
                    path,
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
    recursive: bool,
    output: &mut OutputStream) -> Result<(), io::Error> {
    let mut dirs: Vec<Cell> = Vec::new();
    if arguments.is_empty() {
        dirs.push(Cell::File(
            Box::from(Path::new("."))));
    } else {
        for arg in arguments {
            match &arg.cell {
                Cell::Text(dir) => {
                    dirs.push(Cell::File(Box::from(Path::new(dir))));
                }
                Cell::File(dir) => {
                    dirs.push(arg.cell.clone());
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

    for cell in dirs {
        match cell {
           Cell::File(dir) => run_for_single_directory_or_file(
                dir, recursive, output) ?,
            _ => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Expected a file"))
        }
    }
    return Ok(());
}

fn run_ls(
    _input_type: &Vec<CellType>,
    arguments: &Vec<Argument>,
    _input: &mut InputStream,
    output: &mut OutputStream) -> Result<(), JobError> {
    return to_runtime_error(run_internal(arguments, false, output));
}

fn run_find(
    _input_type: &Vec<CellType>,
    arguments: &Vec<Argument>,
    _input: &mut InputStream,
    output: &mut OutputStream) -> Result<(), JobError> {
    return to_runtime_error(run_internal(arguments, true, output));
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
        run: Some(run_ls),
        mutate: None,
    });
}

pub fn find(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Call, JobError> {
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
        run: Some(run_find),
        mutate: None,
    });
}
