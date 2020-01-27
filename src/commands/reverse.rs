use crate::{
    data::Row,
    stream::{InputStream, OutputStream},
};
use crate::commands::CompileContext;
use crate::errors::{JobResult, error};
use crate::data::Value;
use crate::stream::{RowsReader, Readable};

pub fn run(
    mut input: impl Readable,
    output: OutputStream,
) -> JobResult<()> {
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

pub fn perform(context: CompileContext) -> JobResult<()> {
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
