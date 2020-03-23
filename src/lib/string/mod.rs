use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{command::ExecutionContext, value::ValueType, list::List};
use crate::lib::parse_util::{single_argument, two_arguments, single_argument_field, single_argument_text};
use crate::lang::{value::Value, argument::Argument};
use nix::sys::ptrace::cont;
use crate::lang::command::{CrushCommand, This};
use std::collections::HashMap;
use lazy_static::lazy_static;

mod format;

lazy_static! {
    pub static ref STRING_METHODS: HashMap<Box<str>, Box<CrushCommand + Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<CrushCommand + Send + Sync>> = HashMap::new();
        res.insert(Box::from("upper"), CrushCommand::command(upper, false));
        res.insert(Box::from("lower"), CrushCommand::command(lower, false));
        res.insert(Box::from("split"), CrushCommand::command(split, false));
        res.insert(Box::from("trim"), CrushCommand::command(trim, false));
        res.insert(Box::from("format"), CrushCommand::command(format::format, false));
        res
    };
}

fn upper(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::String(
        context.this.text()?
            .to_uppercase()
            .into_boxed_str()))
}

fn lower(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::String(
        context.this.text()?
            .to_lowercase()
            .into_boxed_str()))
}

fn split(mut context: ExecutionContext) -> CrushResult<()> {
    let this = context.this.text()?;
    let separator = single_argument_text(context.arguments)?;
    context.output.send(Value::List(List::new(ValueType::String,
                                              this.split(separator.as_ref())
                                                  .map(|s| Value::String(Box::from(s)))
                                                  .collect())))
}

fn trim(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::String(
        Box::from(context.this.text()?
            .trim())))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("string")?;
    env.declare("upper", Value::Command(CrushCommand::command(upper, false)))?;
    env.declare("lower", Value::Command(CrushCommand::command(lower, false)))?;
    env.declare("format", Value::Command(CrushCommand::command(format::format, false)))?;
    env.declare("split", Value::Command(CrushCommand::command(split, false)))?;
    env.declare("trim", Value::Command(CrushCommand::command(trim, false)))?;
    env.readonly();
    Ok(())
}
