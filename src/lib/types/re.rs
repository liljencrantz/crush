use crate::lang::errors::{CrushResult, argument_error, error};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use regex::Regex;
use std::error::Error;
use crate::lang::command::CrushCommand;
use crate::lang::execution_context::{ArgumentVector, This};
use std::collections::HashMap;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.insert(Box::from("match"), CrushCommand::command(r#match, false,
            "re =~ input:string", "True if the input matches the pattern", None));
        res.insert(Box::from("not_match"), CrushCommand::command(not_match, false,
            "re !~ input:string", "True if the input does not match the pattern", None));
        res.insert(Box::from("replace"), CrushCommand::command(
            replace, false,
            "re ~ input replacement", "Replace the first match of the regex in the input with the replacement", None));
        res.insert(Box::from("replace_all"), CrushCommand::command(
            replace_all, false,
            "re ~ input replacement", "Replace all matches of the regex in the input with the replacement", None));
        res.insert(Box::from("new"), CrushCommand::command(
            new, false,
            "re:new pattern:string", "Create a new regular expression instance", None));
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
        match (arg.argument_type.as_deref(), arg.value) {
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
        match (arg.argument_type.as_deref(), arg.value) {
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
