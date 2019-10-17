use std::fs;
use crate::stream::{OutputStream, InputStream};
use crate::data::{Cell, CellType, CellDataType, Rows, Row, Argument};
use crate::commands::{Call, to_runtime_error, Exec};
use crate::errors::{JobError, error};
use chrono::{Local, DateTime};
use crate::glob::glob_files;
use std::path::Path;
use std::fs::Metadata;
use crate::state::get_cwd;

fn insert_entity(meta: &Metadata, file: Box<Path>, output: &mut OutputStream) -> Result<(), JobError> {
    let modified_system = to_runtime_error(meta.modified())?;
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
        for maybe_entry in to_runtime_error(dirs)? {
            let entry = to_runtime_error(maybe_entry)?;
            insert_entity(
                &to_runtime_error(entry.metadata())?,
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
                    &to_runtime_error(path.metadata())?,
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

fn run_internal(
    arguments: Vec<Argument>,
    recursive: bool,
    mut output: OutputStream) -> Result<(), JobError> {
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
                    dirs.push(Cell::File(dir.clone()));
                }
                Cell::Glob(dir) => {
                    to_runtime_error(
                        glob_files(
                            dir,
                            Path::new(&get_cwd()?),
                            &mut dirs))?;
                }
                _ => {
                    return Err(error("Invalid argument type to ls, expected string or glob"));
                }
            }
        }
    }

    for cell in dirs {
        match cell {
            Cell::File(dir) => run_for_single_directory_or_file(
                dir, recursive, &mut output)?,
            _ => return Err(error("Expected a file"))
        }
    }
    return Ok(());
}

fn run_ls(
    _input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    _input: InputStream,
    output: OutputStream) -> Result<(), JobError> {
    return run_internal(arguments, false, output);
}

fn run_find(
    _input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    _input: InputStream,
    output: OutputStream) -> Result<(), JobError> {
    return run_internal(arguments, true, output);
}

pub fn ls(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    return Ok(Call {
        name: String::from("ls"),
        input_type,
        arguments,
        output_type: vec![
            CellType {
                name: Some(String::from("file")),
                cell_type: CellDataType::Text,
            },
            CellType {
                name: Some(String::from("size")),
                cell_type: CellDataType::Integer,
            },
            CellType {
                name: Some(String::from("modified")),
                cell_type: CellDataType::Time,
            },
        ],
        exec: Exec::Run(run_ls),
    });
}

pub fn find(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    return Ok(Call {
        name: String::from("ls"),
        input_type,
        arguments,
        output_type: vec![
            CellType {
                name: Some(String::from("file")),
                cell_type: CellDataType::Text,
            },
            CellType {
                name: Some(String::from("size")),
                cell_type: CellDataType::Integer,
            },
            CellType {
                name: Some(String::from("modified")),
                cell_type: CellDataType::Time,
            },
        ],
        exec: Exec::Run(run_find),
    });
}
