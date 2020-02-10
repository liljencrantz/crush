use crate::{
    data::Row,
    stream::{OutputStream},
};
use crate::commands::CompileContext;
use crate::errors::{CrushResult, error};
use crate::data::{Value, RowsReader};
use crate::stream::Readable;

pub fn run(
    mut input: impl Readable,
    output: OutputStream,
) -> CrushResult<()> {
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
    return Ok(());
}

pub fn perform(context: CompileContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Stream(s) => {
            let input = s.stream;
            let output = context.output.initialize(input.get_type().clone())?;
            run(input, output)
        }
        Value::Rows(r) => {
            let input = RowsReader::new(r);
            let output = context.output.initialize(input.get_type().clone())?;
            run(input, output)
        }
        _ => Err(error("Expected a stream")),
    }
}
