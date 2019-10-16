use crate::stream::{OutputStream, InputStream, unlimited_streams};
use crate::cell::{Argument, CellType, Row, CellDataType, Output, Cell};
use crate::commands::{Call, Exec, to_runtime_error};
use crate::errors::{JobError, argument_error};
use crate::glob::glob_files;
use std::io::BufReader;
use std::io::prelude::*;
use std::fs::File;
use std::thread;
use std::path::Path;
use crate::commands::command_util::find_field;
use lazy_static::lazy_static;
use crate::state::get_cwd;

lazy_static! {
    static ref sub_type: Vec<CellType> = {
        vec![CellType {
            name: "line".to_string(),
            cell_type: CellDataType::Text,
        }]
    };
}

fn handle(file: Cell, output: &mut OutputStream) -> Result<(), JobError> {
    let (output_stream, input_stream) = unlimited_streams();
    let out_row = Row {
        cells: vec![
            file.concrete(),
            Cell::Output(Output {
                types: sub_type.clone(),
                stream: input_stream,
            }),
        ],
    };
    output.send(out_row)?;
    thread::spawn(move || {
        let fff = match file {
            Cell::Text(t) => File::open(t),
            Cell::File(b) => File::open(b),
            _ => panic!("Impossible"),
        }.unwrap();
        let mut reader = BufReader::new(&fff);
        let mut line = String::new();
        loop {
            reader.read_line(&mut line);
            if line.is_empty() {
                break;
            }
            output_stream.send(Row { cells: vec![Cell::Text(line[0..line.len() - 1].to_string())] });
            line.clear();
        }
    });
    return Ok(());
}


fn run(
    input_type: Vec<CellType>,
    mut arguments: Vec<Argument>,
    input: InputStream,
    mut output: OutputStream) -> Result<(), JobError> {
    let mut files: Vec<Cell> = Vec::new();
    if (input_type.len() == 0) {
        for arg in &arguments {
            match &arg.cell {
                Cell::Text(_) | Cell::File(_) => files.push(arg.cell.concrete()),
                Cell::Glob(pattern) => to_runtime_error(glob_files(
                    &pattern,
                    Path::new(&get_cwd()?),
                    &mut files,
                ))?,
                _ => return Err(argument_error("Expected a file name")),
            }
        }
        for file in files {
            handle(file, &mut output)?;
        }
    } else {
        if (arguments.len() != 1) {
            return Err(argument_error("Expected one argument: column spec"));
        }
        match &arguments[0].cell {
            Cell::Text(s) | Cell::Field(s) => {
                let idx = find_field(&s, &input_type)?;
                loop {
                    match input.recv() {
                        Ok(row) => handle(row.cells[idx].concrete(), &mut output)?,
                        Err(_) => break,
                    }
                }
            }
            _ => return Err(argument_error("Expected column of type Field")),
        }
    }
    return Ok(());
}

pub fn lines(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    let output_type: Vec<CellType> =
        vec![
            CellType { name: "file".to_string(), cell_type: CellDataType::File },
            CellType { name: "lines".to_string(), cell_type: CellDataType::Output(sub_type.clone()) },
        ];

    return Ok(Call {
        name: String::from("lines"),
        input_type,
        arguments,
        output_type,
        exec: Exec::Run(run),
    });
}
