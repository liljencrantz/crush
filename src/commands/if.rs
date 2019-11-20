use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::errors::error;
use crate::data::ValueType;
use crate::data::Row;
use crate::data::Value;
use crate::data::ColumnType;
use crate::env::get_cwd;

pub fn compile_and_run(mut context: CompileContext) -> JobResult<()> {
    let cc = CompileContext{
        input: context.input,
        output: context.output,
        arguments: vec![],
        env: context.env,
        printer: context.printer,
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
            _ => Err(error("Wrong argument types, expected boolean and closure")),
        }
        3 => match (context.arguments.remove(0).value, context.arguments.remove(0).value, context.arguments.remove(0).value) {
            (Value::Bool(b), Value::Closure(c1), Value::Closure(c2)) => {
                if b {
                    c1.spawn_and_execute(cc)
                } else {
                    c2.spawn_and_execute(cc)
                }
            }
            _ => Err(error("Wrong argument types, expected boolean and two closures")),
        }
        _ => Err(error("Wrong number of arguments")),
    }
}
