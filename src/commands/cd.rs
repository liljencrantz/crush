use crate::cell::{Argument, CellType, Cell, CellDataType};
use crate::commands::{Call, to_runtime_error, Exec};
use crate::errors::JobError;
use crate::state::State;

fn mutate(
    _state: &mut State,
    _input_type: Vec<CellType>,
    arguments: Vec<Argument>) -> Result<(), JobError> {
    return match arguments.len() {
        0 =>
        // This should move to home, not /...
            to_runtime_error(std::env::set_current_dir("/")),
        1 => {
            let dir = &arguments[0];
            return match &dir.cell {
                Cell::Text(val) => to_runtime_error(std::env::set_current_dir(val)),
                _ => Err(JobError { message: String::from("Wrong parameter type, expected text") })
            };
        }
        _ => Err(JobError { message: String::from("Wrong number of arguments") })
    };
}

pub(crate) fn cd(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
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

    return Ok(Call {
        name: String::from("cd"),
        input_type: input_type,
        arguments: arguments,
        output_type: vec![],
        exec: Exec::Mutate(mutate),
    });
}
