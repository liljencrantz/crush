use crate::lang::command::CrushCommand;
use crate::lang::execution_context::{ExecutionContext, This, ArgumentVector};
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::ValueType, dict::Dict};
use crate::lang::value::Value;
use std::collections::HashMap;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.insert(Box::from("new"), CrushCommand::command(
            new, false,
            "dict:new",
            "Construct a new dict",
            Some(r#"    Examples:
    my_dict := ((dict string integer):new)"#)));
        res.insert(Box::from("len"), CrushCommand::command(
            len, false,
            "dict:len",
            "The number of mappings in the dict",
            None));
        res.insert(Box::from("empty"), CrushCommand::command(
            empty, false,
            "dict:empty",
            "True if there are no mappings in the dict",
            None));
        res.insert(Box::from("clear"), Box::from(CrushCommand::command(
            clear, false,
            "dict:clear", "Remove all mappings from this dict", None)));
        res.insert(Box::from("__setitem__"), CrushCommand::command(
            setitem, false,
            "dict[key] = value",
            "Create a new mapping or replace an existing one",
            None));
        res.insert(Box::from("__getitem__"), CrushCommand::command(
            getitem, false,
            "dict[key]",
            "Return the value the specified key is mapped to",
            None));
        res.insert(Box::from("remove"), CrushCommand::command(
            remove, false,
            "dict:remove key",
            "Remove a mapping from the dict",
            None));
        res.insert(Box::from("clone"), Box::from(CrushCommand::command(
            clone, false,
            "dict:clone",
            "Create a new dict with the same st of mappings as this one",
            None)));
        res.insert(Box::from("__call_type__"), CrushCommand::command(
            call_type, false,
            "dict key_type:type value_type:type",
            "Returns a dict type with the specifiec key and value types",
            None));
        res.insert(Box::from("key_type"), CrushCommand::command(
            key_type, false,
            "dict:key_type",
            "Return the type of the keys in this dict",
            None));
        res.insert(Box::from("value_type"), CrushCommand::command(
            value_type, false,
            "dict:value_type",
            "Return the type of the values in this dict",
            None));

        res
    };
}

fn call_type(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let key_type = context.arguments.r#type(0)?;
    let value_type = context.arguments.r#type(1)?;
    context.output.send(Value::Type(ValueType::Dict(Box::new(key_type), Box::new(value_type))))
}

fn new(context: ExecutionContext) -> CrushResult<()> {
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
    let dict = context.this.dict()?;
    let value = context.arguments.value(1)?;
    let key = context.arguments.value(0)?;
    dict.insert(key, value)
}

fn getitem(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let dict = context.this.dict()?;
    let key = context.arguments.value(0)?;
    let o = context.output;
    dict.get(&key).map(|c| o.send(c));
    Ok(())
}

fn remove(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let dict = context.this.dict()?;
    let key = context.arguments.value(0)?;
    let o = context.output;
    dict.remove(&key).map(|c| o.send(c));
    Ok(())
}

fn len(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Integer(context.this.dict()?.len() as i128))
}

fn clear(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let d = context.this.dict()?;
    d.clear();
    context.output.send(Value::Dict(d))
}

fn clone(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let d = context.this.dict()?;
    context.output.send(Value::Dict(d.copy()))
}

fn empty(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Bool(context.this.dict()?.len() == 0))
}

fn key_type(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Type(context.this.dict()?.key_type()))
}

fn value_type(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Type(context.this.dict()?.value_type()))
}
