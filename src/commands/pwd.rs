use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::data::Value;
use crate::env::get_cwd;

pub fn perform(context: CompileContext) -> JobResult<()> {
    context.output.send(Value::File(get_cwd()?))
}
