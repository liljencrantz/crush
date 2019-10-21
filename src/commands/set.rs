use crate::{
    data::{CellType},
    data::Argument,
    commands::{Call, Exec},
    errors::{JobError, argument_error},
    state::State
};
use crate::stream::{InputStream, OutputStream};
use crate::printer::Printer;

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
    state: State,
    printer: Printer,
) -> Result<(), JobError> {
    for arg in arguments {
        state.set(arg.name.unwrap().as_ref(), arg.cell.concrete())?;
    }
    return Ok(());
}

pub fn set(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
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
        exec: Exec::Command(run),
    });
}
