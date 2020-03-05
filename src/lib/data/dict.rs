use crate::lang::command::ExecutionContext;
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::ValueType, dict::Dict, command::SimpleCommand};
use crate::lang::table::Row;
use crate::lang::value::Value;
use crate::lang::table::ColumnType;
use crate::lang::scope::Scope;
use crate::lib::parse_util::single_argument_dict;

fn new(mut context: ExecutionContext) -> CrushResult<()> {
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

fn insert(mut context: ExecutionContext) -> CrushResult<()> {
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

fn get(context: ExecutionContext) -> CrushResult<()> {
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

fn remove(context: ExecutionContext) -> CrushResult<()> {
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

fn len(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Integer(single_argument_dict(context.arguments)?.len() as i128))
}

fn empty(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Bool(single_argument_dict(context.arguments)?.len() == 0))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("dict")?;
    env.declare_str("new", Value::Command(SimpleCommand::new(new, false)))?;
    env.declare_str("insert", Value::Command(SimpleCommand::new(insert, false)))?;
    env.declare_str("get", Value::Command(SimpleCommand::new(get, false)))?;
    env.declare_str("remove", Value::Command(SimpleCommand::new(remove, false)))?;
    env.declare_str("len", Value::Command(SimpleCommand::new(len, false)))?;
    env.declare_str("empty", Value::Command(SimpleCommand::new(empty, false)))?;
    env.readonly();
    Ok(())
}
