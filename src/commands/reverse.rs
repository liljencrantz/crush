use crate::{
    data::Row,
    stream::{OutputStream},
};
use crate::commands::CompileContext;
use crate::errors::{CrushResult, error};
use crate::data::{Value, RowsReader};
use crate::stream::{Readable, ValueSender};

pub fn run(
    mut input: impl Readable,
    sender: ValueSender,
) -> CrushResult<()> {
    let output = sender.initialize(input.get_type().clone())?;
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

pub fn perform(context: CompileContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Stream(s) => run(s.stream, context.output),
        Value::Rows(r) => run(RowsReader::new(r), context.output),
        _ => Err(error("Expected a stream")),
    }
}
