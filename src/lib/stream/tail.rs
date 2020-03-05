use std::collections::VecDeque;

use crate::{
    lang::table::Row,
    lang::stream::OutputStream,
};
use crate::lang::command::ExecutionContext;
use crate::errors::{CrushResult, error};
use crate::lang::stream::{Readable, ValueSender};
use crate::lang::{value::Value, table::TableReader};
use crate::lib::parse_util::{optional_argument_integer};

pub fn run(
    lines: i128,
    mut input: impl Readable,
    sender: ValueSender,
) -> CrushResult<()> {
    let output = sender.initialize(input.types().clone())?;
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

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    let lines = optional_argument_integer(context.arguments)?.unwrap_or(10);
    match context.input.recv()? {
        Value::TableStream(s) => run(lines, s.stream, context.output),
        Value::Table(r) => run(lines, TableReader::new(r), context.output),
        _ => error("Expected a stream"),
    }
}
