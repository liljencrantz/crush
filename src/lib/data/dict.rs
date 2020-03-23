use crate::lang::command::{ExecutionContext, CrushCommand, This};
use crate::lang::errors::{CrushResult, argument_error, error};
use crate::lang::{value::ValueType, dict::Dict};
use crate::lang::table::Row;
use crate::lang::value::Value;
use crate::lang::table::ColumnType;
use crate::lang::scope::Scope;
use crate::lib::parse_util::{single_argument_dict};
use std::collections::HashMap;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref DICT_METHODS: HashMap<Box<str>, Box<CrushCommand + Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<CrushCommand + Send + Sync>> = HashMap::new();
        res.insert(Box::from("len"), Box::from(CrushCommand::command(len, false)));
        res.insert(Box::from("empty"), Box::from(CrushCommand::command(empty, false)));
//        res.insert(Box::from("clear"), Box::from(CrushCommand::command(clear, false)));
        res.insert(Box::from("__setitem__"), Box::from(CrushCommand::command(setitem, false)));
        res.insert(Box::from("__getitem__"), Box::from(CrushCommand::command(getitem, false)));
        res.insert(Box::from("remove"), Box::from(CrushCommand::command(remove, false)));
//        res.insert(Box::from("clone"), Box::from(CrushCommand::command(clone, false)));
        res
    };
}

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
    let mut dict = context.this.dict()?;
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
    let mut dict = context.this.dict()?;
    let key = context.arguments.remove(0).value;
    let output = context.output.initialize(
        vec![ColumnType::new("value", dict.value_type())])?;
    dict.get(&key).map(|c| output.send(Row::new(vec![c])));
    Ok(())
}

fn remove(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() == 0 {
        return argument_error("Missing key")
    }
    let mut dict = context.this.dict()?;
    let key = context.arguments.remove(0).value;
    let o = context.output;
    dict.remove(&key).map(|c| o.send(c));
    Ok(())
}

fn len(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Integer(context.this.dict()?.len() as i128))
}

fn empty(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Bool(context.this.dict()?.len() == 0))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("dict")?;
    env.declare("new", Value::Command(CrushCommand::command(new, false)))?;
    env.readonly();
    Ok(())
}
