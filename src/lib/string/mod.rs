use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{command::ExecutionContext, value::ValueType, list::List};
use crate::lib::parse_util::{single_argument, two_arguments, single_argument_field, single_argument_text};
use crate::lang::{value::Value, command::SimpleCommand, argument::Argument};
use nix::sys::ptrace::cont;
use crate::lang::command::{CrushCommand, This};
use std::collections::HashMap;
use lazy_static::lazy_static;

mod format;

lazy_static! {
    pub static ref STRING_METHODS: HashMap<Box<str>, Box<CrushCommand + Sync>> = {
        let mut res: HashMap<Box<str>, Box<CrushCommand + Sync>> = HashMap::new();
        res.insert(Box::from("upper"), Box::from(SimpleCommand::new(upper, false)));
        res.insert(Box::from("lower"), Box::from(SimpleCommand::new(lower, false)));
        res.insert(Box::from("split"), Box::from(SimpleCommand::new(split, false)));
        res.insert(Box::from("trim"), Box::from(SimpleCommand::new(trim, false)));
        res.insert(Box::from("format"), Box::from(SimpleCommand::new(format::format, false)));
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
    env.declare("upper", Value::Command(SimpleCommand::new(upper, false).boxed()))?;
    env.declare("lower", Value::Command(SimpleCommand::new(lower, false).boxed()))?;
    env.declare("format", Value::Command(SimpleCommand::new(format::format, false).boxed()))?;
    env.declare("split", Value::Command(SimpleCommand::new(split, false).boxed()))?;
    env.declare("trim", Value::Command(SimpleCommand::new(trim, false).boxed()))?;
    env.readonly();
    Ok(())
}
