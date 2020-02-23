use crate::scope::Scope;
use crate::errors::{CrushResult, argument_error};
use crate::lang::ExecutionContext;
use crate::lib::parse_util::{single_argument, two_arguments};
use crate::lang::{Value, SimpleCommand, Argument};
use nix::sys::ptrace::cont;

mod format;

fn upper(mut context: ExecutionContext) -> CrushResult<()> {
    single_argument(&context.arguments)?;
    match context.arguments.remove(0).value {
        Value::Text(t) => context.output.send(Value::Text(t.to_uppercase().into_boxed_str())),
        _ => argument_error("Expected a text argument"),
    }
}

fn lower(mut context: ExecutionContext) -> CrushResult<()> {
    single_argument(&context.arguments)?;
    match context.arguments.remove(0).value {
        Value::Text(t) => context.output.send(Value::Text(t.to_lowercase().into_boxed_str())),
        _ => argument_error("Expected a text argument"),
    }
}

fn split(mut context: ExecutionContext) -> CrushResult<()> {
    two_arguments(&context.arguments)?;



    match context.arguments.remove(0).value {
        Value::Text(t) => context.output.send(Value::Text(t.to_lowercase().into_boxed_str())),
        _ => argument_error("Expected a text argument"),
    }
}


pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("text")?;
    root.uses(&env);
    env.declare_str("upper", Value::Command(SimpleCommand::new(upper)))?;
    env.declare_str("lower", Value::Command(SimpleCommand::new(lower)))?;
    env.declare_str("format", Value::Command(SimpleCommand::new(format::format)))?;
    env.readonly();
    Ok(())
}
