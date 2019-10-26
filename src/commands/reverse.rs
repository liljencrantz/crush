use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::{
    data::Row,
    data::CellDefinition,
    stream::{OutputStream, InputStream},
    data::Argument,
    commands::{Call, Exec},
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
    let output_type = context.input_type.clone();
    Ok((Exec::Command(Box::from(move || run(context.input, context.output))), output_type))
}
