use crate::lib::ExecutionContext;
use crate::errors::CrushResult;

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.arguments.remove(0).value)
}
