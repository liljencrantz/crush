use std::{io, fs};
use crate::stream::{OutputStream, InputStream};
use crate::result::{Argument, CellType, Cell, Row, CellDataType};
use crate::commands::{InternalCall, Command, Call, InternalCommand, to_runtime_error};
use crate::errors::JobError;

#[derive(Clone)]
pub struct Filter {}

struct Check{
    field: u32,
    value: Cell,
}

fn find_field(field: &String, fields: &Vec<CellType>) -> Option

impl InternalCommand for Filter {
    fn run(
        &mut self,
        _input_type: &Vec<CellType>,
        _arguments: &Vec<Argument>,
        input: &mut dyn InputStream,
        output: &mut dyn OutputStream) -> Result<(), JobError> {



        loop {
            match input.next() {
                Some(row) => {
                    if row.cells[0] == _arguments[0].cell {
                        output.add(row);
                    }
                }
                None => {
                    break;
                }
            }
        }
        return Ok(());
    }
}

impl Command for Filter {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Box<dyn Call>, JobError> {
        return Ok(Box::new(InternalCall {
            name: String::from("filter"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type: input_type.clone(),
            command: Box::new(self.clone()),
        }));
    }
}
