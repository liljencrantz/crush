use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, command::ExecutionContext};
use regex::Regex;
use std::error::Error;
use crate::lang::command::{CrushCommand, ArgumentVector, This};
use std::collections::HashMap;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref RE_METHODS: HashMap<Box<str>, Box<CrushCommand + Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<CrushCommand + Send + Sync>> = HashMap::new();
        res.insert(Box::from("match"), CrushCommand::command(r#match, false));
        res.insert(Box::from("not_match"), CrushCommand::command(not_match, false));
        res.insert(Box::from("replace"), CrushCommand::command(replace, false));
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
    let mut all = false;

    for arg in context.arguments.drain(..) {
        match (arg.name.as_deref(), arg.value) {
            (Some("text"), Value::String(t)) => {
                text = Some(t);
            }
            (Some("replacement"), Value::String(t)) => {
                replace = Some(t);
            }
            (Some("all"), Value::Bool(b)) => {
                all = b;
            }
            _ => return argument_error("Invalid argument"),
        }
    }

    match (text, replace) {
        (Some(t), Some(n)) => {
            let txt = if all {
                re.replace_all(t.as_ref(), n.as_ref())
            } else {
                re.replace(t.as_ref(), n.as_ref())
            };
            context.output.send(Value::String(Box::from(txt.as_ref())))
        }
        _ => argument_error("Must specify both pattern and text"),
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("re")?;
    env.declare("new", Value::Command(CrushCommand::command(new, false)))?;
    env.readonly();
    Ok(())
}
