use crate::lang::ExecutionContext;
use crate::errors::{CrushResult, error};
use crate::{
    lang::Argument,
    stream::{OutputStream},
    lang::Value,
    errors::{CrushError, argument_error},
};
use crate::stream::{Readable, ValueSender};
use crate::lang::TableReader;
use crate::lib::parse_util::{optional_argument_integer};

pub fn run(
    lines: i128,
    mut input: impl Readable,
    sender: ValueSender,
) -> CrushResult<()> {
    let output = sender.initialize(input.types().clone())?;
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
        Value::TableStream(s) => run(lines, s.stream, context.output),
        Value::Table(r) => run(lines, TableReader::new(r), context.output),
        _ => error("Expected a stream"),
    }
}
