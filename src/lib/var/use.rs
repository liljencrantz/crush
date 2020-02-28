use crate::lang::{ExecutionContext, Value};
use crate::errors::argument_error;
use crate::errors::CrushResult;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    context.output.initialize(vec![]);

    for arg in context.arguments.iter() {
        match (arg.name.as_deref(), &arg.value) {
            (None, Value::Env(e)) => {
                context.env.uses(e);
            }
            _ => return argument_error("Expected all arguments to be scopes"),
        }
    }
    Ok(())
}
