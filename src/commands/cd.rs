use std::{io, fs};
use crate::stream::{OutputStream, InputStream};
use crate::result::{Argument, CellType, Cell, Row, CellDataType};
use crate::commands::{InternalCall, Command, Call, InternalCommand, to_runtime_error};
use crate::errors::JobError;
use crate::state::State;

#[derive(Clone)]
pub struct Cd {}

impl InternalCommand for Cd {
    fn mutate(
        &mut self,
        _input_type: &Vec<CellType>,
        arguments: &Vec<Argument>,
        _state: &mut State) -> Result<(), JobError> {
        return match arguments.len() {
            0 =>
            // This should move to home, not /...
                to_runtime_error(std::env::set_current_dir("/")),
            1 => {
                let dir = &arguments[0];
                return match &dir.cell {
                    Cell::Text(val) => to_runtime_error(std::env::set_current_dir(val)),
                    _ => Err(JobError { message: String::from("Wrong parameter type, expected text")})
                }
            }
            _ => Err(JobError{ message: String::from("Wrong number of arguments") })
        }
    }
}

impl Command for Cd {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Box<dyn Call>, JobError> {
        if arguments.len() > 1 {
            return Err(JobError {
                message: String::from("Too many arguments")
            });
        }
        if arguments.len() == 1 && arguments[0].cell.cell_data_type() != CellDataType::Text {
            return Err(JobError {
                message: String::from("Wrong argument type, expected text")
            });
        }

        return Ok(Box::new(InternalCall {
            name: String::from("cd"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type: vec![],
            command: Box::new(self.clone()),
        }));
    }
}
