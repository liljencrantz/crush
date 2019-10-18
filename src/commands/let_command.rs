use crate::{
    data::{CellType, Argument},
    commands::{Call, Exec},
    errors::{JobError, argument_error},
    state::State
};

fn mutate(
    state: &mut State,
    _input_type: Vec<CellType>,
    arguments: Vec<Argument>) -> Result<(), JobError> {
    for arg in arguments {
        state.namespace.declare(arg.name.unwrap().as_ref(), arg.cell.concrete())?;
    }
    return Ok(());
}

pub(crate) fn let_command(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    for arg in arguments.iter() {
        if arg.val_or_empty().is_empty() {
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

