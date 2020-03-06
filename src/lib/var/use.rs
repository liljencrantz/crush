use crate::lang::{command::ExecutionContext, value::Value};
use crate::lang::errors::argument_error;
use crate::lang::errors::CrushResult;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    context.output.initialize(vec![]);

    for arg in context.arguments.iter() {
        match (arg.name.as_deref(), &arg.value) {
            (None, Value::Scope(e)) => context.env.r#use(e),
            _ => return argument_error("Expected all arguments to be scopes"),
        }
    }
    Ok(())
}
