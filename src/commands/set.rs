use crate::cell::{Argument, CellType, Cell, CellDataType};
use crate::commands::{Call, to_runtime_error};
use crate::errors::{JobError, argument_error};
use crate::state::State;

fn mutate(
    state: &mut State,
    _input_type: &Vec<CellType>,
    arguments: &Vec<Argument>) -> Result<(), JobError> {
    for arg in arguments {
        state.namespace.set(arg.name.as_str(), arg.cell.clone())?;
    }
    return Ok(());
}

pub(crate) fn set(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Call, JobError> {
    for arg in arguments {
        if arg.name.as_str() == "" {
            return Err(
                argument_error("Missing variable name")
            );
        }
    }

    return Ok(Call {
        name: String::from("set"),
        input_type: input_type.clone(),
        arguments: arguments.clone(),
        output_type: vec![],
        run: None,
        mutate: Some(mutate),
    });
}

