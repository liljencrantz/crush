use crate::{
    data::{Argument, CellDefinition},
    stream::{OutputStream, InputStream},
    data::Cell,
    commands::{Call, Exec},
    errors::{JobError, argument_error}
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::CellFnurp;

pub fn get_line_count(arguments: &Vec<Argument>) -> Result<i128, JobError> {
    return match arguments.len() {
        0 => Ok(10),
        1 => match arguments[0].cell {
            Cell::Integer(v) => Ok(v),
            _ => Err(argument_error("Expected a number"))
        }
        _ => Err(argument_error("Too many arguments"))
    }
}

pub fn run(
    _input_type: Vec<CellFnurp>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    let mut count = 0;
    let tot = get_line_count(&arguments)?;
    loop {
        match input.recv() {
            Ok(row) => {
                if count >= tot {
                    break;
                }
                output.send(row)?;
                count+=1;
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    get_line_count(&arguments)?;
    return Ok(Call {
        name: String::from("head"),
        output_type: input_type.clone(),
        input_type,
        arguments,
        exec: Exec::Command(run),
    });
}
