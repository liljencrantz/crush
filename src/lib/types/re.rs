use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error, error};
use crate::lang::{value::Value, command::ExecutionContext};
use regex::Regex;
use std::error::Error;
use crate::lang::command::{CrushCommand, ArgumentVector, This};
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::value::ValueType;

lazy_static! {
    pub static ref RE_METHODS: HashMap<Box<str>, Box<dyn CrushCommand + Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand + Send + Sync>> = HashMap::new();
        res.insert(Box::from("match"), CrushCommand::command(r#match, false));
        res.insert(Box::from("not_match"), CrushCommand::command(not_match, false));
        res.insert(Box::from("replace"), CrushCommand::command(replace, false));
        res.insert(Box::from("replace_all"), CrushCommand::command(replace_all, false));
        res.insert(Box::from("new"), CrushCommand::command(new, false));
        res
    };
}

fn new(mut context: ExecutionContext) -> CrushResult<()> {
    let def = context.arguments.string(0)?;
    let res = match Regex::new(def.as_ref()) {
        Ok(r) => Value::Regex(def, r),
        Err(e) => return argument_error(e.description()),
    };
    context.output.send(res)
}

fn r#match(mut context: ExecutionContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let needle = context.arguments.string(0)?;
     context.output.send(Value::Bool(re.is_match(&needle)))
}

fn not_match(mut context: ExecutionContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let needle = context.arguments.string(0)?;
    context.output.send(Value::Bool(!re.is_match(&needle)))
}

fn replace(mut context: ExecutionContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let mut text = None;
    let mut replace = None;

    for arg in context.arguments.drain(..) {
        match (arg.name.as_deref(), arg.value) {
            (None, Value::String(t)) => {
                if text.is_none() {
                    text = Some(t);
                } else {
                    if replace.is_none() {
                        replace = Some(t);
                    } else {
                        return error("Too many arguments");
                    }
                }
            }
            (Some("text"), Value::String(t)) => {
                text = Some(t);
            }
            (Some("replacement"), Value::String(t)) => {
                replace = Some(t);
            }
            _ => return argument_error("Invalid argument"),
        }
    }

    match (text, replace) {
        (Some(t), Some(n)) => {
            context.output.send(Value::String(Box::from(re.replace(&t, n.as_ref()).as_ref())))
        }
        _ => argument_error("Must specify both pattern and text"),
    }
}

fn replace_all(mut context: ExecutionContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let mut text = None;
    let mut replace = None;

    for arg in context.arguments.drain(..) {
        match (arg.name.as_deref(), arg.value) {
            (None, Value::String(t)) => {
                if text.is_none() {
                    text = Some(t);
                } else {
                    if replace.is_none() {
                        replace = Some(t);
                    } else {
                        return error("Too many arguments");
                    }
                }
            }
            (Some("text"), Value::String(t)) => {
                text = Some(t);
            }
            (Some("replacement"), Value::String(t)) => {
                replace = Some(t);
            }
            _ => return argument_error("Invalid argument"),
        }
    }

    match (text, replace) {
        (Some(t), Some(n)) => {
            context.output.send(Value::String(Box::from(re.replace_all(&t, n.as_ref()).as_ref())))
        }
        _ => argument_error("Must specify both pattern and text"),
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.declare("re", Value::Type(ValueType::Regex))
}
