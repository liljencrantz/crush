use crate::commands::CompileContext;
use crate::errors::{CrushResult, error};
use crate::{
    data::Argument,
    stream::{OutputStream},
    data::Value,
    errors::{CrushError, argument_error},
};
use crate::stream::Readable;
use crate::data::RowsReader;

pub fn get_line_count(arguments: &Vec<Argument>) -> Result<i128, CrushError> {
    return match arguments.len() {
        0 => Ok(10),
        1 => match arguments[0].value {
            Value::Integer(v) => Ok(v),
            _ => Err(argument_error("Expected a number"))
        }
        _ => Err(argument_error("Too many arguments"))
    };
}

pub fn run(
    lines: i128,
    mut input: impl Readable,
    output: OutputStream,
) -> CrushResult<()> {
    let mut count = 0;
    loop {
        match input.read() {
            Ok(row) => {
                if count >= lines {
                    break;
                }
                output.send(row)?;
                count += 1;
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn perform(context: CompileContext) -> CrushResult<()> {
    let lines = get_line_count(&context.arguments)?;
    match context.input.recv()? {
        Value::Stream(s) => {
            let input = s.stream;
            let output = context.output.initialize(input.get_type().clone())?;
            run(lines, input, output)
        }
        Value::Rows(r) => {
            let input = RowsReader::new(r);
            let output = context.output.initialize(input.get_type().clone())?;
            run(lines, input, output)
        }
        _ => Err(error("Expected a stream")),
    }
}
