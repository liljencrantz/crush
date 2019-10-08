use crate::stream::{OutputStream, InputStream};
use crate::cell::{Argument, CellType, Cell, Row};
use crate::commands::{InternalCall, Command};
use crate::errors::{JobError, argument_error};
use crate::state::State;
use crate::commands::filter::find_field;

#[derive(Clone)]
pub struct Sort {}

impl Command for Sort {
    fn run(
        &mut self,
        _state: &State,
        input_type: &Vec<CellType>,
        arguments: &Vec<Argument>,
        input: &mut dyn InputStream,
        output: &mut dyn OutputStream) -> Result<(), JobError> {
        match (arguments[0].name.as_str(), &arguments[0].cell) {
            ("key", Cell::Text(cell_name)) => {
                let idx = find_field(cell_name, input_type)?;
                let mut res: Vec<Row> = Vec::new();
                loop {
                    match input.next() {
                        Some(row) => {
                            res.push(row);
                        }
                        None => {
                            break;
                        }
                    }
                }
                res.sort_by(|a, b| a.cells[idx].partial_cmp(&b.cells[idx]).expect("OH NO!"));
                for row in &res {
                    output.add(row.clone());
                }

                return Ok(());
            }
            _ => {
                return Err(argument_error("Bad comparison key"));
            }
        }
    }

    fn get_call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<InternalCall, JobError> {
        return Ok(Box::new(InternalCall {
            name: String::from("Sort"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type: input_type.clone(),
            command: Box::new(self.clone()),
        }));
    }
}
