use crate::commands::CompileContext;
use crate::errors::{CrushResult, argument_error};
use crate::env::Env;
use crate::data::{Value, Command};

fn add(mut context: CompileContext) -> CrushResult<()> {
    let mut res = 0i128;
    for a in context.arguments.drain(..) {
        match a.value {
            Value::Integer(i) => res += i,
            _ => return argument_error("Expected integer arguments"),
        }
    }
    context.output.send(Value::Integer(res));
    Ok(())
}

fn mul(mut context: CompileContext) -> CrushResult<()> {
    let mut res = 1i128;
    for a in context.arguments.drain(..) {
        match a.value {
            Value::Integer(i) => res *= i,
            _ => return argument_error("Expected integer arguments"),
        }
    }
    context.output.send(Value::Integer(res));
    Ok(())
}

pub fn declare(root: &Env) -> CrushResult<()> {
    let list = root.create_namespace("math")?;
    list.declare_str("add", Value::Command(Command::new(add)))?;
    list.declare_str("mul", Value::Command(Command::new(mul)))?;
    Ok(())
}
