use crate::lang::command::ExecutionContext;
use crate::lang::errors::{CrushResult, argument_error, error};
use crate::lang::{value::ValueType, list::List, command::SimpleCommand};
use crate::lang::value::Value;
use std::collections::HashSet;
use crate::lib::parse_util::{single_argument_list, single_argument_type, two_arguments, three_arguments, this_list, single_argument_integer};
use crate::lang::scope::Scope;

fn of(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() == 0 {
        return argument_error("Expected at least one element");
    }

    let types = context.arguments.iter().map(|a| a.value.value_type()).collect::<HashSet<ValueType>>();
    let lst = List::new(
        if types.len() == 1 {
            context.arguments[0].value.value_type()
        } else {
            ValueType::Any
        },
        context.arguments.drain(..).map(|a| a.value).collect());
    context.output.send(Value::List(lst))
}

fn new(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::List(List::new(single_argument_type(context.arguments)?, vec![])))
}

fn len(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Integer(this_list(context.this)?.len() as i128))
}

fn empty(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Bool(this_list(context.this)?.len() == 0))
}

fn push(mut context: ExecutionContext) -> CrushResult<()> {
    let l = this_list(context.this)?;
    let mut new_elements: Vec<Value> = Vec::new();
    for el in context.arguments.drain(..) {
        if el.value.value_type() == l.element_type() || l.element_type() == ValueType::Any {
            new_elements.push(el.value)
        } else {
            return argument_error("Invalid element type");
        }
    }
    if !new_elements.is_empty() {
        l.append(&mut new_elements);
    }
    context.output.send(Value::List(l));
    Ok(())
}

fn pop(context: ExecutionContext) -> CrushResult<()> {
    let o = context.output;
    this_list(context.this)?.pop().map(|c| o.send(c));
    Ok(())
}

fn peek(context: ExecutionContext) -> CrushResult<()> {
    let o = context.output;
    this_list(context.this)?.peek().map(|c| o.send(c));
    Ok(())
}

fn clear(context: ExecutionContext) -> CrushResult<()> {
    this_list(context.this)?.clear();
    Ok(())
}

fn set(mut context: ExecutionContext) -> CrushResult<()> {
    two_arguments(&context.arguments)?;
    let mut list = this_list(context.this)?;
    let mut idx = None;
    let mut value = None;

    for arg in context.arguments.drain(..) {
        match (arg.name.as_deref(), arg.value) {
            (Some("index"), Value::Integer(l)) => idx = Some(l),
            (Some("value"), l) => value = Some(l),
            _ => return argument_error("Unexpected argument"),
        }
    }

    match (idx, value) {
        (Some(i), Some(v)) => list.set(i as usize, v),
        _ => argument_error("Missing arguments"),
    }
}

fn remove(mut context: ExecutionContext) -> CrushResult<()> {
    let mut list = this_list(context.this)?;
    let idx = single_argument_integer(context.arguments)?;
    list.remove(idx as usize);
    Ok(())
}

fn truncate(mut context: ExecutionContext) -> CrushResult<()> {
    let mut list = this_list(context.this)?;
    let idx = single_argument_integer(context.arguments)?;
    list.truncate(idx as usize);
    Ok(())
}

fn clone(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::List(this_list(context.this)?.copy()))
}

pub fn list_member(name: &str) -> CrushResult<Value> {
    match name {
        "len" => Ok(Value::Command(SimpleCommand::new(len, false))),
        "empty" => Ok(Value::Command(SimpleCommand::new(empty, false))),
        "push" => Ok(Value::Command(SimpleCommand::new(push, false))),
        "pop" => Ok(Value::Command(SimpleCommand::new(pop, false))),
        "peek" => Ok(Value::Command(SimpleCommand::new(peek, false))),
        "clear" => Ok(Value::Command(SimpleCommand::new(clear, false))),
        "remove" => Ok(Value::Command(SimpleCommand::new(remove, false))),
        "truncate" => Ok(Value::Command(SimpleCommand::new(truncate, false))),
        "clone" => Ok(Value::Command(SimpleCommand::new(clone, false))),
        _ => error(format!("List does not provide a method {}", name).as_str())
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("list")?;
    env.declare("of", Value::Command(SimpleCommand::new(of, false)))?;
    env.declare("new", Value::Command(SimpleCommand::new(new, false)))?;
    env.declare("len", Value::Command(SimpleCommand::new(len, false)))?;
    env.declare("empty", Value::Command(SimpleCommand::new(empty, false)))?;
    env.declare("push", Value::Command(SimpleCommand::new(push, false)))?;
    env.declare("pop", Value::Command(SimpleCommand::new(pop, false)))?;
    env.declare("peek", Value::Command(SimpleCommand::new(peek, false)))?;
    env.declare("set", Value::Command(SimpleCommand::new(set, false)))?;
    env.declare("clear", Value::Command(SimpleCommand::new(clear, false)))?;
    env.declare("remove", Value::Command(SimpleCommand::new(remove, false)))?;
    env.declare("truncate", Value::Command(SimpleCommand::new(truncate, false)))?;
    env.declare("clone", Value::Command(SimpleCommand::new(clone, false)))?;
    env.readonly();
    Ok(())
}
