use crate::lang::ExecutionContext;
use crate::errors::{CrushResult, argument_error};
use crate::lang::{SimpleCommand, Value};
use crate::scope::Scope;
use std::cmp::Ordering;

fn gt(mut context: ExecutionContext) -> CrushResult<()> {
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

fn lt(mut context: ExecutionContext) -> CrushResult<()> {
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

fn lte(mut context: ExecutionContext) -> CrushResult<()> {
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

fn gte(mut context: ExecutionContext) -> CrushResult<()> {
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

fn eq(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    let l = context.arguments.remove(0).value;
    let r = context.arguments.remove(0).value;
    context.output.send(Value::Bool(l.eq(&r)))
}

fn neq(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    let l = context.arguments.remove(0).value;
    let r = context.arguments.remove(0).value;
    context.output.send(Value::Bool(!l.eq(&r)))
}

fn not(mut context: ExecutionContext) -> CrushResult<()> {
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
    root.uses(&env);
    env.declare_str("gt", Value::Command(SimpleCommand::new(gt, false)))?;
    env.declare_str("gte", Value::Command(SimpleCommand::new(gte, false)))?;
    env.declare_str("lt", Value::Command(SimpleCommand::new(lt, false)))?;
    env.declare_str("lte", Value::Command(SimpleCommand::new(lte, false)))?;
    env.declare_str("eq", Value::Command(SimpleCommand::new(eq, false)))?;
    env.declare_str("neq", Value::Command(SimpleCommand::new(neq, false)))?;
    env.declare_str("not", Value::Command(SimpleCommand::new(not, false)))?;
    env.readonly();
    Ok(())
}
