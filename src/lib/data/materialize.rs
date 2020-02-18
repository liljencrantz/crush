use crate::lib::ExecutionContext;
use crate::errors::CrushResult;

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.input.recv()?.materialize())
}
