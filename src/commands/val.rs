use crate::commands::CompileContext;
use crate::errors::CrushResult;

pub fn perform(mut context: CompileContext) -> CrushResult<()> {
    context.output.send(context.arguments.remove(0).value)
}
