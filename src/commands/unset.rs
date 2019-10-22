use crate::{
    data::{CellDefinition},
    data::Argument,
    commands::{Call, Exec},
    errors::{JobError, argument_error},
    env::Env
};
use crate::data::{Cell, CellFnurp};
use crate::printer::Printer;
use crate::stream::{InputStream, OutputStream};

fn run(
    input_type: Vec<CellFnurp>,
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

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
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
