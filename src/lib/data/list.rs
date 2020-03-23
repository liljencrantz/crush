use crate::lang::command::{ExecutionContext, This};
use crate::lang::errors::{CrushResult, argument_error, error};
use crate::lang::{value::ValueType, list::List, command::CrushCommand};
use crate::lang::value::Value;
use std::collections::HashSet;
use std::collections::HashMap;
use crate::lib::parse_util::{single_argument_list, single_argument_type, two_arguments, three_arguments, single_argument_integer};
use crate::lang::scope::Scope;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref LIST_METHODS: HashMap<Box<str>, Box<CrushCommand + Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<CrushCommand + Send + Sync>> = HashMap::new();
        res.insert(Box::from("len"), CrushCommand::command(len, false));
        res.insert(Box::from("empty"), CrushCommand::command(empty, false));
        res.insert(Box::from("push"), CrushCommand::command(push, false));
        res.insert(Box::from("pop"), CrushCommand::command(pop, false));
        res.insert(Box::from("peek"), CrushCommand::command(peek, false));
        res.insert(Box::from("clear"), CrushCommand::command(clear, false));
        res.insert(Box::from("__setitem__"), CrushCommand::command(setitem, false));
        res.insert(Box::from("remove"), CrushCommand::command(remove, false));
        res.insert(Box::from("truncate"), CrushCommand::command(truncate, false));
        res.insert(Box::from("clone"), CrushCommand::command(clone, false));
        res
    };
}

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

fn len(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Integer(context.this.list()?.len() as i128))
}

fn empty(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Bool(context.this.list()?.len() == 0))
}

fn push(mut context: ExecutionContext) -> CrushResult<()> {
    let l = context.this.list()?;
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
    context.this.list()?.pop().map(|c| o.send(c));
    Ok(())
}

fn peek(context: ExecutionContext) -> CrushResult<()> {
    let o = context.output;
    context.this.list()?.peek().map(|c| o.send(c));
    Ok(())
}

fn clear(context: ExecutionContext) -> CrushResult<()> {
    context.this.list()?.clear();
    Ok(())
}

fn setitem(mut context: ExecutionContext) -> CrushResult<()> {
    let mut list = context.this.list()?;
    let value = context.arguments.remove(1).value;
    let key = context.arguments.remove(0).value;

    match key {
        Value::Integer(i) => list.set(i as usize, value),
        _ => argument_error("Missing arguments"),
    }
}

fn remove(mut context: ExecutionContext) -> CrushResult<()> {
    let mut list = context.this.list()?;
    let idx = single_argument_integer(context.arguments)?;
    list.remove(idx as usize);
    Ok(())
}

fn truncate(mut context: ExecutionContext) -> CrushResult<()> {
    let mut list = context.this.list()?;
    let idx = single_argument_integer(context.arguments)?;
    list.truncate(idx as usize);
    Ok(())
}

fn clone(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::List(context.this.list()?.copy()))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("list")?;
    env.declare("of", Value::Command(CrushCommand::command(of, false)))?;
    env.declare("new", Value::Command(CrushCommand::command(new, false)))?;
    env.readonly();
    Ok(())
}
