use crate::lang::ExecutionContext;
use crate::errors::{CrushResult, argument_error};
use crate::lang::{ValueType, List, SimpleCommand};
use crate::lang::Value;
use std::collections::HashSet;
use crate::lib::parse_util::{single_argument_list, single_argument_type, two_arguments, three_arguments};
use crate::scope::Scope;

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
    context.output.send(Value::Integer(single_argument_list(context.arguments)?.len() as i128))
}

fn empty(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Bool(single_argument_list(context.arguments)?.len() == 0))
}

fn push(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() == 0 {
        return argument_error("Expected at least one argument to list.push");
    }
    let cell = context.arguments.remove(0);
    match (&cell.name, &cell.value) {
        (None, Value::List(l)) => {
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
            context.output.send(cell.value);
            Ok(())
        }
        _ => argument_error("Argument is not a list"),
    }
}

fn pop(context: ExecutionContext) -> CrushResult<()> {
    let o = context.output;
    single_argument_list(context.arguments)?.pop().map(|c| o.send(c));
    Ok(())
}

fn peek(context: ExecutionContext) -> CrushResult<()> {
    let o = context.output;
    single_argument_list(context.arguments)?.peek().map(|c| o.send(c));
    Ok(())
}

fn clear(context: ExecutionContext) -> CrushResult<()> {
    single_argument_list(context.arguments)?.clear();
    Ok(())
}

fn set(mut context: ExecutionContext) -> CrushResult<()> {
    three_arguments(&context.arguments)?;
    let mut list = None;
    let mut idx = None;
    let mut value = None;

    for arg in context.arguments.drain(..) {
        match (arg.name.as_deref(), arg.value) {
            (Some("list"), Value::List(l)) => list = Some(l),
            (Some("index"), Value::Integer(l)) => idx = Some(l),
            (Some("value"), l) => value = Some(l),
            _ => return argument_error("Unexpected argument"),
        }
    }

    match (list, idx, value) {
        (Some(l), Some(i), Some(v)) => l.set(i as usize, v),
        _ => argument_error("Missing arguments"),
    }
}

fn remove(mut context: ExecutionContext) -> CrushResult<()> {
    two_arguments(&context.arguments)?;
    let mut list = None;
    let mut idx = None;

    for arg in context.arguments.drain(..) {
        match (arg.name.as_deref(), arg.value) {
            (Some("list"), Value::List(l)) | (None, Value::List(l)) => list = Some(l),
            (Some("index"), Value::Integer(l)) | (None, Value::Integer(l)) => idx = Some(l),
            _ => return argument_error("Unexpected argument"),
        }
    }

    match (list, idx) {
        (Some(l), Some(i)) => l.remove(i as usize),
        _ => return argument_error("Missing arguments"),
    }
    Ok(())
}

fn truncate(mut context: ExecutionContext) -> CrushResult<()> {
    two_arguments(&context.arguments)?;
    let mut list = None;
    let mut idx = None;

    for arg in context.arguments.drain(..) {
        match (arg.name.as_deref(), arg.value) {
            (Some("list"), Value::List(l)) | (None, Value::List(l)) => list = Some(l),
            (Some("index"), Value::Integer(l)) | (None, Value::Integer(l)) => idx = Some(l),
            _ => return argument_error("Unexpected argument"),
        }
    }

    match (list, idx) {
        (Some(l), Some(i)) => l.truncate(i as usize),
        _ => return argument_error("Missing arguments"),
    }
    Ok(())
}

fn clone(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::List(single_argument_list(context.arguments)?.copy()))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("list")?;
    env.declare_str("of", Value::Command(SimpleCommand::new(of)))?;
    env.declare_str("new", Value::Command(SimpleCommand::new(new)))?;
    env.declare_str("len", Value::Command(SimpleCommand::new(len)))?;
    env.declare_str("empty", Value::Command(SimpleCommand::new(empty)))?;
    env.declare_str("push", Value::Command(SimpleCommand::new(push)))?;
    env.declare_str("pop", Value::Command(SimpleCommand::new(pop)))?;
    env.declare_str("peek", Value::Command(SimpleCommand::new(peek)))?;
    env.declare_str("set", Value::Command(SimpleCommand::new(set)))?;
    env.declare_str("clear", Value::Command(SimpleCommand::new(clear)))?;
    env.declare_str("remove", Value::Command(SimpleCommand::new(remove)))?;
    env.declare_str("truncate", Value::Command(SimpleCommand::new(truncate)))?;
    env.declare_str("clone", Value::Command(SimpleCommand::new(clone)))?;
    env.readonly();
    Ok(())
}
