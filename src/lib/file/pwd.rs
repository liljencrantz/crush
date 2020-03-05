use crate::lang::command::ExecutionContext;
use crate::errors::CrushResult;
use crate::lang::value::Value;
use crate::scope::cwd;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::File(cwd()?))
}
