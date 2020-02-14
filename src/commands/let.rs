use crate::commands::CompileContext;
use crate::errors::CrushResult;
use crate::errors::argument_error;

pub fn perform(context: CompileContext) -> CrushResult<()> {
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
