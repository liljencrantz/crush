use crate::lang::command::{ExecutionContext, CrushCommand};
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value};
use crate::lang::scope::Scope;
use std::cmp::Ordering;

pub fn gt(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    let l = context.arguments.remove(0).value;
    let r = context.arguments.remove(0).value;
    match l.partial_cmp(&r) {
        Some(ordering) => context.output.send(Value::Bool(ordering == Ordering::Greater)),
        None => return argument_error("Uncomparable values"),
    }
}

pub fn lt(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    let l = context.arguments.remove(0).value;
    let r = context.arguments.remove(0).value;
    match l.partial_cmp(&r) {
        Some(ordering) => context.output.send(Value::Bool(ordering == Ordering::Less)),
        None => return argument_error("Uncomparable values"),
    }
}

pub fn lte(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    let l = context.arguments.remove(0).value;
    let r = context.arguments.remove(0).value;
    match l.partial_cmp(&r) {
        Some(ordering) => context.output.send(Value::Bool(ordering != Ordering::Greater)),
        None => return argument_error("Uncomparable values"),
    }
}

pub fn gte(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    let l = context.arguments.remove(0).value;
    let r = context.arguments.remove(0).value;
    match l.partial_cmp(&r) {
        Some(ordering) => context.output.send(Value::Bool(ordering != Ordering::Less)),
        None => return argument_error("Uncomparable values"),
    }
}

pub fn eq(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    let l = context.arguments.remove(0).value;
    let r = context.arguments.remove(0).value;
    context.output.send(Value::Bool(l.eq(&r)))
}

pub fn neq(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    let l = context.arguments.remove(0).value;
    let r = context.arguments.remove(0).value;
    context.output.send(Value::Bool(!l.eq(&r)))
}

pub fn not(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 1 {
        return argument_error("Expected exactly one argument");
    }
    match context.arguments.remove(0).value {
        Value::Bool(b) => context.output.send(Value::Bool(!b)),
        _ => argument_error("Expected a boolean argument")
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("comp")?;
    env.declare("gt", Value::Command(CrushCommand::command(gt, false)))?;
    env.declare("gte", Value::Command(CrushCommand::command(gte, false)))?;
    env.declare("lt", Value::Command(CrushCommand::command(lt, false)))?;
    env.declare("lte", Value::Command(CrushCommand::command(lte, false)))?;
    env.declare("eq", Value::Command(CrushCommand::command(eq, false)))?;
    env.declare("neq", Value::Command(CrushCommand::command(neq, false)))?;
    env.declare("not", Value::Command(CrushCommand::command(not, false)))?;
    env.readonly();
    Ok(())
}
