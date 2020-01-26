use crate::commands::CompileContext;
use crate::errors::{JobResult, argument_error};
use crate::data::{ValueType, List};
use crate::data::Value;

pub fn create(context: CompileContext) -> JobResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected 1 argument"));
    }
    match &context.arguments[0].value {
        Value::Text(element_type) => {
            context.output.send(Value::List(List::new(ValueType::from(element_type)?, vec![])))
        }
        _ => Err(argument_error("Invalid argument types")),
    }
}

pub fn len(context: CompileContext) -> JobResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected single argument to list.len"));
    }
    match (&context.arguments[0].name, &context.arguments[0].value) {
        (None, Value::List(l)) => context.output.send(Value::Integer(l.len() as i128)),
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn empty(context: CompileContext) -> JobResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected single argument to list.len"));
    }
    match (&context.arguments[0].name, &context.arguments[0].value) {
        (None, Value::List(l)) => context.output.send(Value::Bool(l.len()==0)),
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn push(mut context: CompileContext) -> JobResult<()> {
    if context.arguments.len() == 0 {
        return Err(argument_error("Expected at least one argument to list.push"));
    }
    let list_cell = context.arguments.remove(0);
    match (&list_cell.name, &list_cell.value) {
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
            context.output.send(list_cell.value);
            Ok(())
        },
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn pop(context: CompileContext) -> JobResult<()> {
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
