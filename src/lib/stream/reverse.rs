use crate::{
    data::Row,
    stream::{OutputStream},
};
use crate::lib::ExecutionContext;
use crate::errors::{CrushResult, error};
use crate::data::{Value, RowsReader};
use crate::stream::{Readable, ValueSender};

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
        Value::Stream(s) => run(s.stream, context.output),
        Value::Rows(r) => run(RowsReader::new(r), context.output),
        _ => error("Expected a stream"),
    }
}
