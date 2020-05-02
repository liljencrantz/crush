use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::execution_context::JobContext;
use crate::lang::parser::parse;
use crate::lang::printer::Printer;
use crate::lang::scope::Scope;
use crate::lang::stream::{empty_channel, ValueSender};
use std::fs;
use std::path::Path;

pub fn file(
    global_env: Scope,
    filename: &Path,
    printer: &Printer,
    output: &ValueSender,
) -> CrushResult<()> {
    let cmd = to_crush_error(fs::read_to_string(filename))?;

    string(global_env, &cmd.as_str(), printer, output);
    Ok(())
}

pub fn string(global_env: Scope, s: &str, printer: &Printer, output: &ValueSender) {
    match parse(s, &global_env) {
        Ok(jobs) => {
            for job_definition in jobs {
                match job_definition.invoke(JobContext::new(
                    empty_channel(),
                    output.clone(),
                    global_env.clone(),
                    printer.clone(),
                )) {
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
}
