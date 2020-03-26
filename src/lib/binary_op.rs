use crate::lang::command::{ExecutionContext, CrushCommand};
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::scope::Scope;
use crate::lang::{value::Value};
use chrono::Duration;

macro_rules! binary_op {
    ($name:ident, $this_type:ident, $($input_type:ident, $output_type:ident, $operation:expr), *) => {
fn $name(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let this = context.this.$this_type()?;
    match (context.arguments.value(0)?) {
        $( Value::$input_type(v) => context.output.send(Value::$output_type($operation(this, v))), )*
        _ => return argument_error("Expected only arguments of the same type"),
    }
}
    }
}
/*
fn tadd(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
        (Value::Time(i1), Value::Duration(i2)) => context.output.send(Value::Time(i1+i2)),
        _ => return argument_error("Expected only arguments of the same type"),
    }
}

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

fn dmul(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
        (Value::Duration(i1), Value::Integer(i2)) => context.output.send(Value::Duration(i1*(i2 as i32))),
        _ => return argument_error("Expected only arguments of the same type"),
    }
}

fn ddiv(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
        (Value::Duration(i1), Value::Integer(i2)) => context.output.send(Value::Duration(i1/(i2 as i32))),
        _ => return argument_error("Expected only arguments of the same type"),
    }
}


fn neg(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 1 {
        return argument_error("Expected exactly one arguments");
    }
    match context.arguments.remove(0).value {
        Value::Duration(v) => context.output.send(Value::Duration(-v)),
        Value::Integer(v) => context.output.send(Value::Integer(-v)),
        Value::Float(v) => context.output.send(Value::Float(-v)),
        Value::Duration(v) => context.output.send(Value::Duration(-v)),
        _ => return argument_error("Bad argument type"),
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("math")?;
    env.declare("add", Value::Command(CrushCommand::command(add, false)))?;
    env.declare("sub", Value::Command(CrushCommand::command(sub, false)))?;
    env.declare("mul", Value::Command(CrushCommand::command(mul, false)))?;
    env.declare("div", Value::Command(CrushCommand::command(div, false)))?;
    env.declare("neg", Value::Command(CrushCommand::command(neg, false)))?;
    env.readonly();
    Ok(())
}
*/
