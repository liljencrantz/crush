use std::{io, fs};
use crate::stream::{OutputStream, InputStream};
use crate::result::{Argument, CellType, Cell, Row, CellDataType};
use crate::commands::{InternalCall, Command, Call, InternalCommand, to_runtime_error};
use crate::errors::JobError;
use chrono::{Local, DateTime};
use crate::state::State;

#[derive(Clone)]
pub struct Ls {}

impl Ls {
    fn run_internal(
        &mut self,
        _input_type: &Vec<CellType>,
        _arguments: &Vec<Argument>,
        _input: &mut dyn InputStream, output: &mut dyn OutputStream) -> Result<(), io::Error> {
        let dirs = fs::read_dir(".");
        for maybe_entry in dirs? {
            let entry = maybe_entry?;
            let meta = entry.metadata()?;
            let modified_system = meta.modified()?;
            let modified_datetime:DateTime<Local> = DateTime::from(modified_system);
            match entry.file_name().into_string() {
                Ok(name) =>
                    output.add(Row {
                        cells: vec![
                            Cell::Text(name),
                            Cell::Integer(i128::from(meta.len())),
                            Cell::Time(modified_datetime),
                        ]
                    }),
                _ => {}
            }
        }
        Ok(())
    }
}

impl Command for Ls {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Box<dyn Call>, JobError> {
        return Ok(Box::new(InternalCall {
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
            command: Box::new(self.clone()),
        }));
    }
}

impl InternalCommand for Ls {
    fn run(
        &mut self,
        _state: &State,
        _input_type: &Vec<CellType>,
        _arguments: &Vec<Argument>,
        _input: &mut dyn InputStream,
        output: &mut dyn OutputStream) -> Result<(), JobError> {
        return to_runtime_error(self.run_internal(_input_type, _arguments, _input, output));
    }
}
