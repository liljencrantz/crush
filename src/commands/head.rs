use crate::{
    data::{Argument, CellDefinition},
    stream::{OutputStream, InputStream},
    data::Cell,
    commands::{Call, Exec},
    errors::{JobError, argument_error},
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::CellFnurp;

pub fn get_line_count(arguments: Vec<Argument>) -> Result<i128, JobError> {
    return match arguments.len() {
        0 => Ok(10),
        1 => match arguments[0].cell {
            Cell::Integer(v) => Ok(v),
            _ => Err(argument_error("Expected a number"))
        }
        _ => Err(argument_error("Too many arguments"))
    };
}

pub struct Config {
    pub lines: i128,
    pub input: InputStream,
    pub output: OutputStream,
}

pub fn run(
    config: Config,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    let mut count = 0;
    loop {
        match config.input.recv() {
            Ok(row) => {
                if count >= config.lines {
                    break;
                }
                config.output.send(row)?;
                count += 1;
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    Ok((Exec::Head(Config {
        lines: get_line_count(arguments)?,
        input,
        output
    }), input_type))
}
