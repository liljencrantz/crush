use crate::lang::command::{ExecutionContext, This};
use crate::lang::errors::CrushResult;
use crate::lang::errors::error;
use crate::lang::value::Value;
use crate::lang::command::CrushCommand;

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    let cc = ExecutionContext {
        input: context.input,
        output: context.output,
        arguments: vec![],
        env: context.env,
        this: None,
    };
    match context.arguments.len() {
        2 => match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
            (Value::Bool(b), Value::Command(c)) => {
                if b {
                    c.invoke(cc)
                } else {
                    cc.output.initialize(vec![])?;
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
