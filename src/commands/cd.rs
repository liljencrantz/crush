use crate::commands::CompileContext;
use crate::errors::{to_job_error, JobResult, error};
use std::path::Path;
use crate::data::Cell;

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let dir = match context.arguments.len() {
        0 => Ok(Box::from(Path::new("/"))),
        1 => {
            let dir = &context.arguments[0];
            match &dir.cell {
                Cell::Text(val) => Ok(Box::from(Path::new(val.as_ref()))),
                Cell::File(val) => Ok(val.clone()),
                _ => Err(error("Wrong parameter type, expected text"))
            }
        }
        _ => Err(error("Wrong number of arguments"))
    }?;
    context.output.initialize(vec![]);
    to_job_error(std::env::set_current_dir(dir))
}
