use crate::lang::command::{ExecutionContext, This, ArgumentVector};
use crate::lang::errors::{CrushResult, argument_error, error};
use crate::lang::{value::ValueType, list::List, command::CrushCommand};
use crate::lang::value::Value;
use std::collections::HashSet;
use std::collections::HashMap;
use crate::lang::scope::Scope;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand + Sync + Send>> = {
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
        res.insert(Box::from("of"), CrushCommand::command(of, false));
        res.insert(Box::from("new"), CrushCommand::command(new, false));
        res.insert(Box::from("fnurp"), CrushCommand::command(fnurp, false));
        res
    };
}

fn fnurp(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context.output.send(Value::Type(ValueType::List(Box::new(context.arguments.r#type(0)?))))
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
    context.arguments.check_len(1)?;
    context.output.send(Value::List(List::new(context.arguments.r#type(0)?, vec![])))
}

fn len(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Integer(context.this.list()?.len() as i128))
}

fn empty(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
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
    context.arguments.check_len(0)?;
    let o = context.output;
    context.this.list()?.pop().map(|c| o.send(c));
    Ok(())
}

fn peek(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let o = context.output;
    context.this.list()?.peek().map(|c| o.send(c));
    Ok(())
}

fn clear(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.this.list()?.clear();
    Ok(())
}

fn setitem(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let list = context.this.list()?;
    let key = context.arguments.integer(0)?;
    let value = context.arguments.value(1)?;
    list.set(key as usize, value)
}

fn remove(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let list = context.this.list()?;
    let idx = context.arguments.integer(0)?;
    list.remove(idx as usize);
    Ok(())
}

fn truncate(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let list = context.this.list()?;
    let idx = context.arguments.integer(0)?;
    list.truncate(idx as usize);
    Ok(())
}

fn clone(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::List(context.this.list()?.copy()))
}
