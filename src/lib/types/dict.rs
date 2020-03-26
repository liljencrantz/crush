use crate::lang::command::{ExecutionContext, CrushCommand, This, ArgumentVector};
use crate::lang::errors::{CrushResult, argument_error, error};
use crate::lang::{value::ValueType, dict::Dict};
use crate::lang::table::Row;
use crate::lang::value::Value;
use crate::lang::table::ColumnType;
use crate::lang::scope::Scope;
use std::collections::HashMap;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<CrushCommand + Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand + Send + Sync>> = HashMap::new();
        res.insert(Box::from("new"), CrushCommand::command(new, false));
        res.insert(Box::from("len"), CrushCommand::command(len, false));
        res.insert(Box::from("empty"), CrushCommand::command(empty, false));
//        res.insert(Box::from("clear"), Box::from(CrushCommand::command(clear, false)));
        res.insert(Box::from("__setitem__"), CrushCommand::command(setitem, false));
        res.insert(Box::from("__getitem__"), CrushCommand::command(getitem, false));
        res.insert(Box::from("remove"), CrushCommand::command(remove, false));
//        res.insert(Box::from("clone"), Box::from(CrushCommand::command(clone, false)));
        res
    };
}

fn new(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let t = context.this.r#type()?;
    if let ValueType::Dict(key_type, value_type) = t {
        if !key_type.is_hashable() {
            argument_error("Key type is not hashable")
        } else {
            context.output.send(Value::Dict(Dict::new(*key_type, *value_type)))
        }
    } else {
        argument_error("Expected a dict type as this value")
    }
}

fn setitem(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let mut dict = context.this.dict()?;
    let value = context.arguments.value(1)?;
    let key = context.arguments.value(0)?;
    if dict.key_type() == key.value_type() && dict.value_type() == value.value_type() {
        dict.insert(key, value);
        Ok(())
    } else {
        argument_error("Wrong key/value type")
    }
}

fn getitem(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let mut dict = context.this.dict()?;
    let key = context.arguments.value(0)?;
    let o = context.output;
    dict.get(&key).map(|c| o.send(c));
    Ok(())
}

fn remove(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let mut dict = context.this.dict()?;
    let key = context.arguments.value(0)?;
    let o = context.output;
    dict.remove(&key).map(|c| o.send(c));
    Ok(())
}

fn len(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Integer(context.this.dict()?.len() as i128))
}

fn empty(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Bool(context.this.dict()?.len() == 0))
}
