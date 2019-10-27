use crate::commands::CompileContext;
use crate::errors::argument_error;
use crate::errors::JobResult;

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    context.output.initialize(vec![]);

    for arg in context.arguments.iter() {
        if arg.val_or_empty().is_empty() {
            return Err(
                argument_error("Missing variable name")
            );
        }
    }
    for arg in context.arguments {
        context.env.set(arg.name.unwrap().as_ref(), arg.cell)?;
    }
    return Ok(());
}
