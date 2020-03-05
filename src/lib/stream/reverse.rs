use crate::{
    lang::table::Row,
    lang::stream::{OutputStream},
};
use crate::lang::command::ExecutionContext;
use crate::errors::{CrushResult, error};
use crate::lang::{value::Value, table::TableReader};
use crate::lang::stream::{Readable, ValueSender};

pub fn run(
    mut input: impl Readable,
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
    match context.input.recv()? {
        Value::TableStream(s) => run(s.stream, context.output),
        Value::Table(r) => run(TableReader::new(r), context.output),
        _ => error("Expected a stream"),
    }
}
