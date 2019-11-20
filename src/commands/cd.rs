use crate::commands::CompileContext;
use crate::errors::{to_job_error, JobResult, error};
use std::path::Path;
use crate::data::Value;
use crate::env::get_home;

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let dir = match context.arguments.len() {
        0 => get_home(),
        1 => {
            let dir = &context.arguments[0];
            match &dir.value {
                Value::Text(val) => Ok(Box::from(Path::new(val.as_ref()))),
                Value::File(val) => Ok(val.clone()),
                _ => Err(error("Wrong parameter type, expected text"))
            }
        }
        _ => Err(error("Wrong number of arguments"))
    }?;
    context.output.initialize(vec![])?;
    to_job_error(std::env::set_current_dir(dir))
}
