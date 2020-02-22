use crate::lang::ExecutionContext;
use crate::errors::CrushResult;
use crate::lang::Value;
use crate::scope::cwd;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::File(cwd()?))
}
