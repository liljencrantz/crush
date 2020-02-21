use crate::lib::ExecutionContext;
use crate::errors::CrushResult;
use crate::errors::error;
use crate::data::Value;

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    let cc = ExecutionContext {
        input: context.input,
        output: context.output,
        arguments: vec![],
        env: context.env,
        printer: context.printer,
        is_loop: false,
    };
    match context.arguments.len() {
        2 => match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
            (Value::Bool(b), Value::Closure(c)) => {
                if b {
                    c.spawn_and_execute(cc)
                } else {
                    cc.output.initialize(vec![])?;
                    Ok(())
                }
            }
            _ => error("Wrong argument types, expected boolean and closure"),
        }
        3 => match (context.arguments.remove(0).value, context.arguments.remove(0).value, context.arguments.remove(0).value) {
            (Value::Bool(b), Value::Closure(c1), Value::Closure(c2)) => {
                if b {
                    c1.spawn_and_execute(cc)
                } else {
                    c2.spawn_and_execute(cc)
                }
            }
            _ => error("Wrong argument types, expected boolean and two closures"),
        }
        _ => error("Wrong number of arguments"),
    }
}
