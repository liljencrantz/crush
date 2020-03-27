use crate::lang::table::Row;
use crate::lang::command::ExecutionContext;
use crate::lang::errors::{CrushResult, error};
use crate::lang::stream::{Readable, ValueSender};

pub fn run(
    input: &mut dyn Readable,
    sender: ValueSender,
) -> CrushResult<()> {
    let output = sender.initialize(input.types().clone())?;
    let mut q: Vec<Row> = Vec::new();
    loop {
        match input.read() {
            Ok(row) => {
                q.push(row);
            }
            Err(_) => {
                loop {
                    if q.is_empty() {
                        break;
                    }
                    output.send(q.pop().unwrap())?;
                }
                break;
            }
        }
    }
    Ok(())
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(mut input) => run(input.as_mut(), context.output),
        None => error("Expected a stream"),
    }
}
