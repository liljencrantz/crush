use crate::lang::errors::{CrushResult, to_crush_error};
use crate::lang::printer::Printer;
use crate::lang::scope::Scope;
use std::fs;
use crate::lang::parser::parse;
use crate::lang::execution_context::JobContext;
use crate::lang::stream::{empty_channel, ValueSender};
use std::path::Path;

pub fn file(global_env: Scope, filename: &Path, printer: &Printer, output: &ValueSender) -> CrushResult<()> {
    let cmd = to_crush_error(fs::read_to_string(filename))?;
    match parse(&cmd.as_str(), &global_env) {
        Ok(jobs) => {
            for job_definition in jobs {
                match job_definition.invoke(JobContext::new(
                    empty_channel(), output.clone(), global_env.clone(), printer.clone())) {
                    Ok(handle) => {
                        handle.join(&printer);
                    }
                    Err(e) => printer.crush_error(e),
                }
            }
        }
        Err(error) => {
            printer.crush_error(error);
        }
    }
    Ok(())
}
