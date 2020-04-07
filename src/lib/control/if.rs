use crate::lang::execution_context::{ExecutionContext};
use crate::lang::errors::CrushResult;
use crate::lang::errors::error;
use crate::lang::value::Value;

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    let cc = ExecutionContext {
        input: context.input,
        output: context.output,
        arguments: vec![],
        env: context.env,
        this: None,
        printer: context.printer
    };
    match context.arguments.len() {
        2 => match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
            (Value::Bool(b), Value::Command(c)) => {
                if b {
                    c.invoke(cc)
                } else {
                    Ok(())
                }
            }
            _ => error("Wrong argument types, expected boolean and closure"),
        }
        3 => match (context.arguments.remove(0).value, context.arguments.remove(0).value, context.arguments.remove(0).value) {
            (Value::Bool(b), Value::Command(c1), Value::Command(c2)) => {
                if b {
                    c1.invoke(cc)
                } else {
                    c2.invoke(cc)
                }
            }
            _ => error("Wrong argument types, expected boolean and two closures"),
        }
        _ => error("Wrong number of arguments"),
    }
}
