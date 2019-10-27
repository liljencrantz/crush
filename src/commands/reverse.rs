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
    input: InputStream,
    output: OutputStream,
) -> JobResult<()> {
    let mut q: Vec<Row> = Vec::new();
    loop {
        match input.recv() {
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

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize()?;
    let output = context.output.initialize(input.get_type().clone())?;
    run(input, output)
}
