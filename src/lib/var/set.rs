use crate::lang::command::ExecutionContext;
use crate::lang::errors::argument_error;
use crate::lang::errors::CrushResult;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    context.output.initialize(vec![]);

    for arg in context.arguments.iter() {
        if arg.val_or_empty().is_empty() {
            return argument_error("Missing variable name");
        }
    }
    for arg in context.arguments {
        context.env.set_str(arg.name.unwrap().as_ref(), arg.value)?;
    }
    Ok(())
}
