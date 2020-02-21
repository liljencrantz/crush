use crate::lib::ExecutionContext;
use crate::errors::{CrushResult, argument_error};
use crate::data::{Command, Value};
use crate::namespace::Namespace;
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

pub fn declare(root: &Namespace) -> CrushResult<()> {
    let env = root.create_namespace("comp")?;
    root.uses(&env);
    env.declare_str("gt", Value::Command(Command::new(gt)))?;
    env.declare_str("gte", Value::Command(Command::new(gte)))?;
    env.declare_str("lt", Value::Command(Command::new(lt)))?;
    env.declare_str("lte", Value::Command(Command::new(lte)))?;
    env.declare_str("eq", Value::Command(Command::new(eq)))?;
    env.declare_str("neq", Value::Command(Command::new(neq)))?;
    env.declare_str("not", Value::Command(Command::new(not)))?;
    Ok(())
}
