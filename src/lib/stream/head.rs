use crate::lib::ExecutionContext;
use crate::errors::{CrushResult, error};
use crate::{
    data::Argument,
    stream::{OutputStream},
    data::Value,
    errors::{CrushError, argument_error},
};
use crate::stream::{Readable, ValueSender};
use crate::data::RowsReader;
use crate::lib::parse_util::{optional_argument_integer};

pub fn run(
    lines: i128,
    mut input: impl Readable,
    sender: ValueSender,
) -> CrushResult<()> {
    let output = sender.initialize(input.get_type().clone())?;
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

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    let lines = optional_argument_integer(context.arguments)?.unwrap_or(10);
    match context.input.recv()? {
        Value::Stream(s) => run(lines, s.stream, context.output),
        Value::Rows(r) => run(lines, RowsReader::new(r), context.output),
        _ => error("Expected a stream"),
    }
}
