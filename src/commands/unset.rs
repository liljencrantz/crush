use crate::{
    data::{CellType},
    data::Argument,
    commands::{Call, Exec},
    errors::{JobError, argument_error},
    env::Env
};
use crate::data::Cell;
use crate::printer::Printer;
use crate::stream::{InputStream, OutputStream};

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    for arg in arguments {
        if let Cell::Text(s) = arg.cell {
            env.remove(&s);
        }
    }
    return Ok(());
}

pub fn unset(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    for arg in arguments.iter() {
        if let Cell::Text(s) = &arg.cell {
            if s.len() == 0 {
                return Err(argument_error("Illegal variable name"));
            }
        } else {
            return Err(argument_error("Illegal variable name"));
        }
    }

    return Ok(Call {
        name: String::from("unset"),
        input_type,
        arguments,
        output_type: vec![],
        exec: Exec::Command(run),
    });
}
