use crate::scope::Scope;
use crate::errors::{CrushResult, argument_error};
use crate::lang::{Value, SimpleCommand, ExecutionContext};
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
            (Some("text"), Value::Text(t)) | (None, Value::Text(t)) => {
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
            (Some("text"), Value::Text(t)) => {
                text = Some(t);
            }
            (Some("replacement"), Value::Text(t)) => {
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
            context.output.send(Value::Text(Box::from(txt.as_ref())))
        }
        _ => argument_error("Must specify both pattern and text"),
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("re")?;
    env.declare_str("new", Value::Command(SimpleCommand::new(new)))?;
    env.declare_str("match", Value::Command(SimpleCommand::new(r#match)))?;
    env.declare_str("replace", Value::Command(SimpleCommand::new(replace)))?;
    env.readonly();
    Ok(())
}
