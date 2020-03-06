use crate::lang::command::ExecutionContext;
use crate::lang::errors::{CrushResult, error};
use crate::{
    lang::argument::Argument,
    lang::stream::{OutputStream},
    lang::value::Value,
    lang::errors::{CrushError, argument_error},
};
use crate::lang::stream::{Readable, ValueSender};
use crate::lang::table::TableReader;
use crate::lib::parse_util::{optional_argument_integer};

pub fn run(
    lines: i128,
    input: &mut dyn Readable,
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
    match context.input.recv()?.readable() {
        Some(mut r) => run(lines, r.as_mut(), context.output),
        None => error("Expected a stream"),
    }
}
