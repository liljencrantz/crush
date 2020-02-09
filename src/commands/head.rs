use crate::commands::CompileContext;
use crate::errors::{JobResult, error};
use crate::{
    data::Argument,
    stream::{OutputStream},
    data::Value,
    errors::{JobError, argument_error},
};
use crate::stream::Readable;
use crate::data::RowsReader;

pub fn get_line_count(arguments: &Vec<Argument>) -> Result<i128, JobError> {
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
) -> JobResult<()> {
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

pub fn perform(context: CompileContext) -> JobResult<()> {
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
