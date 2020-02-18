use crate::lib::ExecutionContext;
use crate::errors::{CrushResult, argument_error};
use crate::namepspace::Namespace;
use crate::data::{Value, Command};
use chrono::Duration;

macro_rules! combine_many {
    ($name:ident, $type:ident, $identity:expr, $operation:expr) => {
fn $name(mut context: ExecutionContext) -> CrushResult<()> {
    let mut res = $identity;
    for a in context.arguments.drain(..) {
        match a.value {
            Value::$type(i) => {res = $operation(res, i)},
            _ => return argument_error("Expected only arguments of the same type"),
        }
    }
    context.output.send(Value::$type(res))
}
    }
}

macro_rules! combine_two {
    ($name:ident, $type:ident, $operation:expr) => {
fn $name(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
        (Value::$type(i1), Value::$type(i2)) => context.output.send(Value::$type($operation(i1, i2))),
        _ => return argument_error("Expected only arguments of the same type"),
    }
}
    }
}

macro_rules! combine_functions {
    ($name:ident, $( $type:ident => $func:ident), *) => {
fn $name(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() == 0 {
        return argument_error("Expected at least one argument");
    }
    match context.arguments[0].value {
        $( Value::$type(i) => $func(context), )*
        _ => argument_error("Can not process arguments of specified type"),
    }
}
    }
}


combine_many!(iadd, Integer, 0i128, |a, b| a+b);
combine_many!(fadd, Float, 0.0, |a, b| a+b);
combine_many!(dadd, Duration, Duration::seconds(0), |a, b| a+b);

fn tadd(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
        (Value::Time(i1), Value::Duration(i2)) => context.output.send(Value::Time(i1+i2)),
        _ => return argument_error("Expected only arguments of the same type"),
    }
}

combine_two!(isub, Integer, |a, b| a-b);
combine_two!(fsub, Float, |a, b| a-b);
combine_two!(dsub, Duration, |a, b| a-b);

fn tsub(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
        (Value::Time(i1), Value::Duration(i2)) => context.output.send(Value::Time(i1-i2)),
        (Value::Time(i1), Value::Time(i2)) => context.output.send(Value::Duration(i1-i2)),
        _ => return argument_error("Unexpected argument type"),
    }
}

combine_many!(imul, Integer, 1i128, |a, b| a*b);
combine_many!(fmul, Float, 1.0, |a, b| a*b);
fn dmul(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
        (Value::Duration(i1), Value::Integer(i2)) => context.output.send(Value::Duration(i1*(i2 as i32))),
        _ => return argument_error("Expected only arguments of the same type"),
    }
}

combine_two!(idiv, Integer, |a, b| a/b);
combine_two!(fdiv, Float, |a, b| a/b);
fn ddiv(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
        (Value::Duration(i1), Value::Integer(i2)) => context.output.send(Value::Duration(i1/(i2 as i32))),
        _ => return argument_error("Expected only arguments of the same type"),
    }
}

combine_functions!(add, Integer => iadd, Float => fadd, Duration => dadd, Time => tadd);
combine_functions!(sub, Integer => isub, Float => fsub, Duration => dsub, Time => tsub);
combine_functions!(mul, Integer => imul, Float => fmul, Duration => dmul);
combine_functions!(div, Integer => idiv, Float => fdiv, Duration => ddiv);

pub fn declare(root: &Namespace) -> CrushResult<()> {
    let env = root.create_namespace("math")?;
    root.uses(&env);
    env.declare_str("add", Value::Command(Command::new(add)))?;
    env.declare_str("sub", Value::Command(Command::new(sub)))?;
    env.declare_str("mul", Value::Command(Command::new(mul)))?;
    env.declare_str("div", Value::Command(Command::new(div)))?;
    Ok(())
}
