use crate::lib::ExecutionContext;
use crate::errors::{CrushResult, argument_error, to_job_error};
use crate::data::{Value, Command, BinaryReader};
use crate::scope::cwd;


pub fn cmd(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() == 0 {
        return argument_error("No command given");
    }
    match context.arguments.remove(0).value {
        Value::Text(cmd) => {
            let mut cmd = std::process::Command::new(cmd.as_ref());
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
