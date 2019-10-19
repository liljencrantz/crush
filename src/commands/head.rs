use crate::{
    data::{Argument, CellType},
    stream::{OutputStream, InputStream},
    data::Cell,
    commands::{Call, Exec},
    errors::{JobError, argument_error}
};
use crate::printer::Printer;

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

fn run(
    _input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
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

pub fn head(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    get_line_count(&arguments)?;
    return Ok(Call {
        name: String::from("head"),
        output_type: input_type.clone(),
        input_type,
        arguments,
        exec: Exec::Run(run),
    });
}
