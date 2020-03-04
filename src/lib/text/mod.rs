use crate::scope::Scope;
use crate::errors::{CrushResult, argument_error};
use crate::lang::{ExecutionContext, ValueType, List};
use crate::lib::parse_util::{single_argument, two_arguments, single_argument_field, single_argument_text};
use crate::lang::{Value, SimpleCommand, Argument};
use nix::sys::ptrace::cont;

mod format;

fn upper(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Text(
        single_argument_text(context.arguments)?
            .to_uppercase()
            .into_boxed_str()))
}

fn lower(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Text(
        single_argument_text(context.arguments)?
            .to_lowercase()
            .into_boxed_str()))
}

fn split(mut context: ExecutionContext) -> CrushResult<()> {
    two_arguments(&context.arguments)?;

    let mut separator = None;
    let mut text = None;

    for arg in context.arguments.drain(..) {
        match (arg.name.as_deref(), arg.value) {
            (Some("separator"), Value::Text(t)) => {
                if t.len() != 1 {
                    return argument_error("Separator must be single character");
                }
                separator = Some(t.chars().next().unwrap());
            }
            (Some("text"), Value::Text(t)) => text = Some(t),
            _ => return argument_error("Unknown argument"),
        }
    }

    match (separator, text) {
        (Some(s), Some(t)) => {
            context.output.send(Value::List(List::new(ValueType::Text,
                                                      t.split(s)
                                                          .map(|s| Value::Text(Box::from(s)))
                                                          .collect())))
        }
        _ => argument_error("Missing arguments")
    }
}

fn trim(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Text(
        Box::from(single_argument_text(context.arguments)?
            .trim())))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("text")?;
    env.declare_str("upper", Value::Command(SimpleCommand::new(upper, false)))?;
    env.declare_str("lower", Value::Command(SimpleCommand::new(lower, false)))?;
    env.declare_str("format", Value::Command(SimpleCommand::new(format::format, false)))?;
    env.declare_str("split", Value::Command(SimpleCommand::new(split, false)))?;
    env.declare_str("trim", Value::Command(SimpleCommand::new(trim, false)))?;
    env.readonly();
    Ok(())
}
