use crate::{
    data::Row,
    stream::{InputStream, OutputStream},
};
use crate::commands::CompileContext;
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

pub fn perform(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize_stream()?;
    let output = context.output.initialize(input.get_type().clone())?;
    run(input, output)
}
