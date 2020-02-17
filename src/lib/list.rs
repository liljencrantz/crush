use crate::lib::ExecutionContext;
use crate::errors::{CrushResult, argument_error};
use crate::data::{ValueType, List, Command};
use crate::data::Value;
use std::collections::HashSet;
use crate::lib::parse_util::{single_argument_list, single_argument_type};
use crate::env::Env;

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

fn create(mut context: ExecutionContext) -> CrushResult<()> {
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

pub fn declare(root: &Env) -> CrushResult<()> {
    let list = root.create_namespace("list")?;
    list.declare_str("of", Value::Command(Command::new(of)))?;
    list.declare_str("create", Value::Command(Command::new(create)))?;
    list.declare_str("len", Value::Command(Command::new(len)))?;
    list.declare_str("empty", Value::Command(Command::new(empty)))?;
    list.declare_str("push", Value::Command(Command::new(push)))?;
    list.declare_str("pop", Value::Command(Command::new(pop)))?;
    Ok(())
}
