use crate::lang::command::ExecutionContext;
use crate::lang::errors::CrushResult;
use crate::lang::errors::argument_error;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    for arg in context.arguments.iter() {
        if arg.val_or_empty().is_empty() {
            return argument_error("Missing variable name");
        }
    }
    for arg in context.arguments {
        context.env.declare_str(arg.name.unwrap().as_ref(), arg.value)?;
    }
    Ok(())
}
