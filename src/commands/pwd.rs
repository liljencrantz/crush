use crate::stream::{OutputStream, InputStream};
use crate::result::{Argument, CellType, Cell, Row, CellDataType};
use crate::commands::{InternalCall, Command, Call, InternalCommand};
use crate::errors::JobError;
use crate::state::State;

#[derive(Clone)]
pub struct Pwd {}

impl InternalCommand for Pwd {
    fn run(
        &mut self,
        _state: &State,
        _input_type: &Vec<CellType>,
        _arguments: &Vec<Argument>,
        _input: &mut dyn InputStream,
        output: &mut dyn OutputStream) -> Result<(), JobError> {
        return match std::env::current_dir() {
            Ok(os_dir) => {
                match os_dir.to_str() {
                    Some(dir) => output.add(Row {
                        cells: vec![Cell::Text(String::from(dir))]
                    }),
                    None => {}
                }
                Ok(())
            },
            Err(io_err) =>
                Err(JobError{ message: io_err.to_string() }),
        };
    }
}

impl Command for Pwd {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Box<dyn Call>, JobError> {
        return Ok(Box::new(InternalCall {
            name: String::from("pwd"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type: vec![CellType {
                name: String::from("directory"),
                cell_type: CellDataType::Text,
            }],
            command: Box::new(self.clone()),
        }));
    }
}
