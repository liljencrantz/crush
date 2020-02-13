use std::collections::VecDeque;

use crate::{
    data::Row,
    stream::OutputStream,
};
use crate::commands::CompileContext;
use crate::errors::{CrushResult, error};
use crate::stream::Readable;
use crate::data::{Value, RowsReader};
use crate::commands::parse_util::single_argument_integer;

pub fn run(
    lines: i128,
    mut input: impl Readable,
    output: OutputStream,
) -> CrushResult<()> {
    let mut q: VecDeque<Row> = VecDeque::new();
    loop {
        match input.read() {
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

pub fn perform(context: CompileContext) -> CrushResult<()> {
    let lines = single_argument_integer(context.arguments)?;
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
