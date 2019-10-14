use crate::cell::{Argument, CellType};
use crate::commands::{Call, Exec};
use crate::errors::{JobError, argument_error};
use crate::state::State;

fn mutate(
    state: &mut State,
    _input_type: Vec<CellType>,
    arguments: Vec<Argument>) -> Result<(), JobError> {
    for arg in arguments {
        state.namespace.declare(arg.name.as_str(), arg.cell.concrete())?;
    }
    return Ok(());
}

pub(crate) fn let_command(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    for arg in arguments.iter() {
        if arg.name.as_str() == "" {
            return Err(
                argument_error("Missing variable name")
            );
        }
    }

    return Ok(Call {
        name: String::from("set"),
        input_type,
        arguments,
        output_type: vec![],
        exec: Exec::Mutate(mutate),
    });
}

