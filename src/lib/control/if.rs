use crate::lang::errors::CrushResult;
use crate::lang::execution_context::{ArgumentVector, ExecutionContext};
use crate::lang::value::Value;

fn execute_or_send(value: Value, context: ExecutionContext) -> CrushResult<()> {
    match value {
        Value::Command(cmd) => cmd.invoke(context),
        v => context.output.send(v),
    }
}

pub fn r#if(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len_range(2, 3)?;
    let b = context.arguments.bool(0)?;
    if b {
        execute_or_send(context.arguments.value(1)?, context.with_args(vec![], None))
    } else {
        context
            .arguments
            .optional_value(2)?
            .map(|v| execute_or_send(v, context.with_args(vec![], None)))
            .unwrap_or(Ok(()))
    }
}
