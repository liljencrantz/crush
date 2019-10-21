use crate::{
    data::{CellType, Argument},
    commands::{Call, Exec},
    errors::{JobError, argument_error},
    env::Env
};
use crate::stream::{OutputStream, InputStream};
use crate::printer::Printer;

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    for arg in arguments {
        env.declare(arg.name.unwrap().as_ref(), arg.cell)?;
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
        exec: Exec::Command(run),
    });
}

