use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::{
    data::Row,
    data::CellDefinition,
    stream::{OutputStream, InputStream},
    data::Argument,
    errors::{JobError, argument_error},
    commands::head::get_line_count,
};
use std::collections::VecDeque;
use crate::printer::Printer;
use crate::env::Env;
use crate::data::ColumnType;
use crate::commands::head;


pub fn run(
    lines: i128,
    input: InputStream,
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

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let lines = get_line_count(&context.arguments)?;
    let input = context.input.initialize()?;
    let output = context.output.initialize(input.get_type().clone())?;
    run(lines, input, output)
}
