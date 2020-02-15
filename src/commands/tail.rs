use std::collections::VecDeque;

use crate::{
    data::Row,
    stream::OutputStream,
};
use crate::commands::CompileContext;
use crate::errors::{CrushResult, error};
use crate::stream::{Readable, ValueSender};
use crate::data::{Value, RowsReader};
use crate::commands::parse_util::{optional_argument_integer};

pub fn run(
    lines: i128,
    mut input: impl Readable,
    sender: ValueSender,
) -> CrushResult<()> {
    let output = sender.initialize(input.get_type().clone())?;
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
    let lines = optional_argument_integer(context.arguments)?.unwrap_or(10);
    match context.input.recv()? {
        Value::Stream(s) => run(lines, s.stream, context.output),
        Value::Rows(r) => run(lines, RowsReader::new(r), context.output),
        _ => error("Expected a stream"),
    }
}
