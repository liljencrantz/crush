use crate::lang::command::ExecutionContext;
use crate::lang::errors::{CrushResult, argument_error, error};
use crate::lang::{value::ValueType, dict::Dict, command::SimpleCommand};
use crate::lang::table::Row;
use crate::lang::value::Value;
use crate::lang::table::ColumnType;
use crate::lang::scope::Scope;
use crate::lib::parse_util::{single_argument_dict, this_dict};

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

fn setitem(mut context: ExecutionContext) -> CrushResult<()> {
    let mut dict = this_dict(context.this)?;
    let value = context.arguments.remove(1).value;
    let key = context.arguments.remove(0).value;
    if dict.key_type() == key.value_type() && dict.value_type() == value.value_type() {
        dict.insert(key, value);
        Ok(())
    } else {
        argument_error("Wrong key/value type")
    }
}

fn getitem(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() == 0 {
        return argument_error("Missing key")
    }
    let mut dict = this_dict(context.this)?;
    let key = context.arguments.remove(0).value;
    let output = context.output.initialize(
        vec![ColumnType::named("value", dict.value_type())])?;
    dict.get(&key).map(|c| output.send(Row::new(vec![c])));
    Ok(())
}

fn remove(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() == 0 {
        return argument_error("Missing key")
    }
    let mut dict = this_dict(context.this)?;
    let key = context.arguments.remove(0).value;
    let o = context.output;
    dict.remove(&key).map(|c| o.send(c));
    Ok(())
}

fn len(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Integer(this_dict(context.this)?.len() as i128))
}

fn empty(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Bool(this_dict(context.this)?.len() == 0))
}

pub fn dict_member(name: &str) -> CrushResult<Value> {
    match name {
        "len" => Ok(Value::Command(SimpleCommand::new(len, false))),
        "setitem" => Ok(Value::Command(SimpleCommand::new(setitem, false))),
        "getitem" => Ok(Value::Command(SimpleCommand::new(getitem, false))),
        "empty" => Ok(Value::Command(SimpleCommand::new(empty, false))),
//        "clear" => Ok(Value::Command(SimpleCommand::new(clear, false))),
        "remove" => Ok(Value::Command(SimpleCommand::new(remove, false))),
//        "clone" => Ok(Value::Command(SimpleCommand::new(clone, false))),
        _ => error(format!("Dict does not provide a method {}", name).as_str())
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("dict")?;
    env.declare("new", Value::Command(SimpleCommand::new(new, false)))?;
    env.declare("setitem", Value::Command(SimpleCommand::new(setitem, false)))?;
    env.declare("getitem", Value::Command(SimpleCommand::new(getitem, false)))?;
    env.declare("remove", Value::Command(SimpleCommand::new(remove, false)))?;
    env.declare("len", Value::Command(SimpleCommand::new(len, false)))?;
    env.declare("empty", Value::Command(SimpleCommand::new(empty, false)))?;
    env.readonly();
    Ok(())
}
