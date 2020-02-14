use crate::commands::CompileContext;
use crate::errors::{CrushResult, argument_error};
use crate::data::{ValueType, Dict, Command};
use crate::data::Row;
use crate::data::Value;
use crate::data::ColumnType;
use crate::env::Env;
use crate::commands::parse_util::single_argument_dict;

fn create(mut context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected 2 arguments to dict.create");
    }
    let first_type = context.arguments.remove(0).value;
    let second_type = context.arguments.remove(0).value;

    match (first_type, second_type) {
        (Value::Type(key_type), Value::Type(value_type)) => {
            if !key_type.is_hashable() {
                return argument_error("Key type is not hashable");
            }
            context.output.send(Value::Dict(Dict::new(key_type, value_type)))
        }
        _ => argument_error("Invalid argument types"),
    }
}

fn insert(mut context: CompileContext) -> CrushResult<()> {
    let output = context.output.initialize(vec![])?;
    if context.arguments.len() != 3 {
        return argument_error("Expected three arguments");
    }
    let value = context.arguments.remove(2).value;
    let key = context.arguments.remove(1).value;
    match &context.arguments[0].value {
        Value::Dict(dict) => {
            if dict.key_type() == key.value_type() && dict.value_type() == value.value_type() {
                dict.insert(key, value);
                Ok(())
            } else {
                argument_error("Wrong key/value type")
            }
        }
        _ => argument_error("Argument is not a dict"),
    }
}

fn get(context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected two arguments");
    }
    match &context.arguments[0].value {
        Value::Dict(dict) => {
            let output = context.output.initialize(
                vec![ColumnType::named("value", dict.value_type())])?;
            dict.get(&context.arguments[1].value).map(|c| output.send(Row::new(vec![c])));
            Ok(())
        }
        _ => argument_error("Argument is not a list"),
    }
}

fn remove(context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected two arguments");
    }
    match &context.arguments[0].value {
        Value::Dict(dict) => {
            dict.remove(&context.arguments[1].value).map(|c| context.output.send(c));
            Ok(())
        }
        _ => argument_error("Argument is not a dict"),
    }
}

fn len(context: CompileContext) -> CrushResult<()> {
    context.output.send(Value::Integer(single_argument_dict(context.arguments)?.len() as i128))
}

fn empty(context: CompileContext) -> CrushResult<()> {
    context.output.send(Value::Bool(single_argument_dict(context.arguments)?.len() == 0))
}

pub fn declare(root: &Env) -> CrushResult<()> {
    let dict = root.create_namespace("dict")?;
    dict.declare_str("create", Value::Command(Command::new(create)))?;
    dict.declare_str("insert", Value::Command(Command::new(insert)))?;
    dict.declare_str("get", Value::Command(Command::new(get)))?;
    dict.declare_str("remove", Value::Command(Command::new(remove)))?;
    dict.declare_str("len", Value::Command(Command::new(len)))?;
    dict.declare_str("empty", Value::Command(Command::new(empty)))?;
    Ok(())
}
