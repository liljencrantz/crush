use crate::lang::command::ExecutionContext;
use crate::lang::errors::CrushResult;
use crate::lang::value::Value;
use crate::util::file::cwd;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::File(cwd()?))
}
