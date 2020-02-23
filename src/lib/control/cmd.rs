use crate::lang::ExecutionContext;
use crate::errors::{CrushResult, argument_error, to_job_error};
use crate::lang::{Value, SimpleCommand, BinaryReader};
use crate::scope::cwd;

pub fn cmd(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() == 0 {
        return argument_error("No command given");
    }
    match context.arguments.remove(0).value {
        Value::File(f) => {
            let mut cmd = std::process::Command::new(f.as_os_str());
            for a in context.arguments.drain(..) {
                cmd.arg(a.value.to_string());
            }
            let output = to_job_error(cmd.output())?;
            context.output.send(
                Value::BinaryStream(
                    BinaryReader::vec(&output.stdout)))
        }
        _ => argument_error("Not a valid command")
    }
}
