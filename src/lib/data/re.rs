use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, command::SimpleCommand, command::ExecutionContext};
use crate::lib::parse_util::single_argument_text;
use regex::Regex;
use std::error::Error;

fn new(mut context: ExecutionContext) -> CrushResult<()> {
    let def = single_argument_text(context.arguments)?;
    let res = match Regex::new(def.as_ref()) {
        Ok(r) => Value::Regex(def, r),
        Err(e) => return argument_error(e.description()),
    };
    context.output.send(res)
}

fn r#match(mut context: ExecutionContext) -> CrushResult<()> {
    let mut re = None;
    let mut needle = None;

    for arg in context.arguments.drain(..) {
        match (arg.name.as_deref(), arg.value) {
            (Some("re"), Value::Regex(s, r)) | (None, Value::Regex(s, r)) => {
                re = Some(r);
            }
            (Some("text"), Value::String(t)) | (None, Value::String(t)) => {
                needle = Some(t);
            }
            _ => return argument_error("Invalid argument"),
        }
    }

    match (re, needle) {
        (Some(r), Some(t)) => {
            context.output.send(Value::Bool(r.is_match(t.as_ref())))
        }
        _ => argument_error("Must specify both pattern and text"),
    }
}

fn replace(mut context: ExecutionContext) -> CrushResult<()> {
    let mut re = None;
    let mut text = None;
    let mut replace = None;
    let mut all = false;

    for arg in context.arguments.drain(..) {
        match (arg.name.as_deref(), arg.value) {
            (Some("re"), Value::Regex(s, r)) | (None, Value::Regex(s, r)) => {
                re = Some(r);
            }
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

    match (re, text, replace) {
        (Some(r), Some(t), Some(n)) => {
            let txt = if all {
                r.replace_all(t.as_ref(), n.as_ref())
            } else {
                r.replace(t.as_ref(), n.as_ref())
            };
            context.output.send(Value::String(Box::from(txt.as_ref())))
        }
        _ => argument_error("Must specify both pattern and text"),
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("re")?;
    env.declare("new", Value::Command(SimpleCommand::new(new, false)))?;
    env.declare("match", Value::Command(SimpleCommand::new(r#match, false)))?;
    env.declare("replace", Value::Command(SimpleCommand::new(replace, false)))?;
    env.readonly();
    Ok(())
}
