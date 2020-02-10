use crate::commands::CompileContext;
use crate::errors::{CrushResult, argument_error};
use crate::data::Value;

pub fn to(mut context: CompileContext) -> CrushResult<()> {
    match context.arguments.len() {
        1 => {
            let a = context.arguments.remove(0);
            match (a.name, a.value) {
                (None, Value::Type(new_type)) => context.output.send(context.input.recv()?.cast(new_type)?),
                _ => return Err(argument_error("Expected argument type")),
            }
        }
        _ => Err(argument_error("Expected exactly one argument")),
    }
}

pub fn of(mut context: CompileContext) -> CrushResult<()> {
    context.output.send(Value::Type(context.input.recv()?.value_type()))
}
