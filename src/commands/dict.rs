use crate::commands::CompileContext;
use crate::errors::{CrushResult, argument_error};
use crate::data::{ValueType, Dict};
use crate::data::Row;
use crate::data::Value;
use crate::data::ColumnType;

pub fn create(context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return Err(argument_error("Expected 2 arguments to dict.create"));
    }
    match (&context.arguments[0].value, &context.arguments[1].value) {
        (Value::Text(key_type), Value::Text(value_type)) => {
            let key_type = ValueType::from(key_type)?;
            if !key_type.is_hashable() {
                return Err(argument_error("Key type is not hashable"));
            }
            context.output.send(Value::Dict(Dict::new(key_type, ValueType::from(value_type)?)))
        }
        _ => Err(argument_error("Invalid argument types")),
    }
}

pub fn insert(mut context: CompileContext) -> CrushResult<()> {
    let output = context.output.initialize(vec![])?;
    if context.arguments.len() != 3 {
        return Err(argument_error("Expected three arguments"));
    }
    let value = context.arguments.remove(2).value;
    let key = context.arguments.remove(1).value;
    match &context.arguments[0].value {
        Value::Dict(dict) => {
            if dict.key_type() == key.value_type() && dict.value_type() == value.value_type() {
                dict.insert(key, value);
                Ok(())
            } else {
                Err(argument_error("Wrong key/value type"))
            }
        }
        _ => Err(argument_error("Argument is not a dict")),
    }
}

pub fn get(context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return Err(argument_error("Expected two arguments"));
    }
    match &context.arguments[0].value {
        Value::Dict(dict) => {
            let output = context.output.initialize(
                vec![ColumnType::named("value", dict.value_type())])?;
            dict.get(&context.arguments[1].value).map(|c| output.send(Row::new(vec![c])));
            Ok(())
        }
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn remove(context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return Err(argument_error("Expected two arguments"));
    }
    match &context.arguments[0].value {
        Value::Dict(dict) => {
            dict.remove(&context.arguments[1].value).map(|c| context.output.send(c));
            Ok(())
        }
        _ => Err(argument_error("Argument is not a dict")),
    }
}

pub fn len(context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected one argument"));
    }
    match &context.arguments[0].value {
        Value::Dict(dict) => {
            context.output.send(Value::Integer(dict.len() as i128))
        }
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn empty(context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected one argument"));
    }
    match &context.arguments[0].value {
        Value::Dict(dict) => {
            context.output.send(Value::Bool(dict.len() == 0))
        }
        _ => Err(argument_error("Argument is not a list")),
    }
}
