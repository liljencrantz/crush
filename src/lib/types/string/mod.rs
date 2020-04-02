use crate::lang::errors::CrushResult;
use crate::lang::{execution_context::ExecutionContext, value::ValueType, list::List};
use crate::lang::value::Value;
use crate::lang::execution_context::{This, ArgumentVector};
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::command::CrushCommand;

mod format;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.insert(Box::from("upper"), CrushCommand::command(
            upper, false,
            "string:upper",
            "Returns an identical string but in upper case",
            None));
        res.insert(Box::from("lower"), CrushCommand::command(
            lower, false,
            "string:lower",
            "Returns an identical string but in lower case",
            None));
        res.insert(Box::from("split"), CrushCommand::command(
            split, false,
            "string:split separator:string",
            "Splits a string using the specifiec separator",
            None));
        res.insert(Box::from("trim"), CrushCommand::command(
            trim, false,
            "string:trim",
            "Returns a string with all whitespace trimmed from both ends",
            None));
        res.insert(Box::from("format"), CrushCommand::command(
            format::format, false,
            "string:format pattern:string [parameters:any]...",
            "Format arguments into a string",
            None));
        res
    };
}

fn upper(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::String(
        context.this.string()?
            .to_uppercase()
            .into_boxed_str()))
}

fn lower(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::String(
        context.this.string()?
            .to_lowercase()
            .into_boxed_str()))
}

fn split(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let this = context.this.string()?;
    let separator = context.arguments.string(0)?;
    context.output.send(Value::List(List::new(ValueType::String,
                                              this.split(separator.as_ref())
                                                  .map(|s| Value::string(s))
                                                  .collect())))
}

fn trim(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::String(
        Box::from(context.this.string()?
            .trim())))
}
