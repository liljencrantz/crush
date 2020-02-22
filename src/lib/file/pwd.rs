use crate::lib::ExecutionContext;
use crate::errors::CrushResult;
use crate::data::Value;
use crate::scope::cwd;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::File(cwd()?))
}
