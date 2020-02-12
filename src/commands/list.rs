use crate::commands::CompileContext;
use crate::errors::{CrushResult, argument_error};
use crate::data::{ValueType, List};
use crate::data::Value;
use std::collections::HashSet;

pub fn of(mut context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() == 0 {
        return Err(argument_error("Expected at least one element"));
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

pub fn create(mut context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected 1 argument"));
    }
    match context.arguments.remove(0).value {
        Value::Type(element_type) => {
            context.output.send(Value::List(List::new(element_type, vec![])))
        }
        _ => Err(argument_error("Invalid argument types")),
    }
}

pub fn len(context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected single argument to list.len"));
    }
    match (&context.arguments[0].name, &context.arguments[0].value) {
        (None, Value::List(l)) => context.output.send(Value::Integer(l.len() as i128)),
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn empty(context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected single argument to list.len"));
    }
    match (&context.arguments[0].name, &context.arguments[0].value) {
        (None, Value::List(l)) => context.output.send(Value::Bool(l.len() == 0)),
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn push(mut context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() == 0 {
        return Err(argument_error("Expected at least one argument to list.push"));
    }
    let cell = context.arguments.remove(0);
    match (&cell.name, &cell.value) {
        (None, Value::List(l)) => {
            let mut new_elements: Vec<Value> = Vec::new();
            for el in context.arguments.drain(..) {
                if el.value.value_type() == l.element_type() {
                    new_elements.push(el.value)
                } else {
                    return Err(argument_error("Invalid element type"));
                }
            }
            if !new_elements.is_empty() {
                l.append(&mut new_elements);
            }
            context.output.send(cell.value);
            Ok(())
        }
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn pop(context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected single argument to list.len"));
    }
    match (&context.arguments[0].name, &context.arguments[0].value) {
        (None, Value::List(l)) => {
            l.pop().map(|c| context.output.send(c));
            Ok(())
        }
        _ => Err(argument_error("Argument is not a list")),
    }
}
