use crate::stream::{OutputStream, InputStream};
use crate::result::{Argument, CellType, Cell};
use crate::commands::{InternalCall, Command, Call, InternalCommand};
use crate::errors::JobError;
use crate::state::State;

#[derive(Clone)]
pub struct Filter {}

struct Check<'a>{
    idx: usize,
    value: &'a Cell,
}

fn find_field(needle: &String, haystack: &Vec<CellType>) -> Result<usize, JobError> {
    for (idx, field) in haystack.iter().enumerate() {
        if field.name.eq(needle) {
            return Ok(idx);
        }
    }
    return Err(JobError {message: String::from(format!("Unknown column \"{}\"", needle))});
}

fn find_checks<'a>(input_type: &Vec<CellType>,
               arguments: &'a Vec<Argument>) -> Result<Vec<Check<'a>>, JobError> {
    let mut res: Vec<Check> = Vec::new();
    for arg in arguments {
        let idx = find_field(&arg.name, input_type)?;
        if arg.cell.cell_data_type() != input_type[idx].cell_type {
            return Err(JobError {message: String::from("Mismatching cell types")});
        }
        res.push(Check {idx, value: &arg.cell});
    }
    return Ok(res);
}

impl InternalCommand for Filter {
    fn run(
        &mut self,
        _state: &State,
        input_type: &Vec<CellType>,
        arguments: &Vec<Argument>,
        input: &mut dyn InputStream,
        output: &mut dyn OutputStream) -> Result<(), JobError> {

        let checks = find_checks(input_type, arguments)?;

        loop {
            match input.next() {
                Some(row) => {
                    let mut ok = false;
                    for check in &checks {
                        if row.cells[check.idx].eq(check.value) {
                            ok = true;
                            break;
                        }
                    }

                    if ok {
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
