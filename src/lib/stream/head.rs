use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use crate::lang::errors::{CrushResult, error};
use crate::lang::stream::{Readable, ValueSender};

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

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    let lines = context.arguments.optional_integer(0)?.unwrap_or(10);
    match context.input.recv()?.readable() {
        Some(mut r) => run(lines, r.as_mut(), context.output),
        None => error("Expected a stream"),
    }
}
