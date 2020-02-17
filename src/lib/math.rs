use crate::lib::ExecutionContext;
use crate::errors::{CrushResult, argument_error};
use crate::env::Env;
use crate::data::{Value, Command};

macro_rules! combine_many_integers {
    ($name:ident, $identity:expr, $operation:expr) => {
fn $name(mut context: ExecutionContext) -> CrushResult<()> {
    let mut res: i128 = $identity;
    for a in context.arguments.drain(..) {
        match a.value {
            Value::Integer(i) => {res = $operation(res, i)},
            _ => return argument_error("Expected only arguments"),
        }
    }
    context.output.send(Value::Integer(res));
    Ok(())
}
    }
}

macro_rules! combine_many_floats {
    ($name:ident, $identity:expr, $operation:expr) => {
fn $name(mut context: ExecutionContext) -> CrushResult<()> {
    let mut res: f64 = $identity;
    for a in context.arguments.drain(..) {
        match a.value {
            Value::Float(i) => {res = $operation(res, i)},
            _ => return argument_error("Expected only arguments"),
        }
    }
    context.output.send(Value::Float(res));
    Ok(())
}
    }
}

macro_rules! combine_many {
    ($name:ident, $iname:ident, $fname:ident) => {
fn $name(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() == 0 {
        return argument_error("Expcted at least one argument");
    }
    match context.arguments[0].value {
        Value::Integer(i) => $iname(context),
        Value::Float(f) => $fname(context),
        _ => argument_error("Can not add arguments of type"),
    }
}
    }
}


combine_many_integers!(iadd, 0i128, |a, b| a+b);
combine_many_integers!(imul, 1i128, |a, b| a*b);

combine_many_floats!(fadd, 0.0, |a, b| a+b);
combine_many_floats!(fmul, 1.0, |a, b| a*b);

combine_many!(add, iadd, fadd);
combine_many!(mul, imul, fmul);

pub fn declare(root: &Env) -> CrushResult<()> {
    let list = root.create_namespace("math")?;
    list.declare_str("add", Value::Command(Command::new(add)))?;
    list.declare_str("mul", Value::Command(Command::new(mul)))?;
    Ok(())
}
