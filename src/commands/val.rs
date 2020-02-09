use crate::commands::CompileContext;
use crate::errors::JobResult;

pub fn perform(mut context: CompileContext) -> JobResult<()> {
    context.output.send(context.arguments.remove(0).value)
}
