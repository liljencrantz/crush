use std::collections::VecDeque;

use crate::{
    commands::head::get_line_count,
    data::Row,
    stream::{InputStream, OutputStream},
};
use crate::commands::CompileContext;
use crate::errors::{JobResult, error};
use crate::stream::{RowsReader, Readable};
use crate::data::Value;

pub fn run(
    lines: i128,
    input: impl Readable,
    output: OutputStream,
) -> JobResult<()> {
    let mut q: VecDeque<Row> = VecDeque::new();
    loop {
        match input.recv() {
            Ok(row) => {
                if q.len() >= lines as usize {
                    q.pop_front();
                }
                q.push_back(row);
            }
            Err(_) => {
                for row in q.drain(..) {
                    output.send(row)?;
                }
                break;
            }
        }
    }
    return Ok(());
}

pub fn perform(context: CompileContext) -> JobResult<()> {
    let lines = get_line_count(&context.arguments)?;
    match context.input.recv()? {
        Value::Stream(s) => {
            let input = s.stream;
            let output = context.output.initialize(input.get_type().clone())?;
            run(lines, input, output)
        }
        Value::Rows(r) => {
            let input = RowsReader::new(r);
            let output = context.output.initialize(input.get_type().clone())?;
            run(lines, input, output)
        }
        _ => Err(error("Expected a stream")),
    }
}
