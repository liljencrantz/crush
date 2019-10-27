use crate::{
    commands::head::get_line_count,
    data::Argument,
    data::CellDefinition,
    data::Row,
    errors::{argument_error, JobError},
    stream::{InputStream, OutputStream},
};
use crate::commands::CompileContext;
use crate::commands::head;
use crate::data::ColumnType;
use crate::errors::JobResult;

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
