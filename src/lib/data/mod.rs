use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, command::SimpleCommand, r#struct::Struct};
use crate::lang::command::{ExecutionContext, CrushCommand};
use crate::lib::parse_util::three_arguments;
use crate::lang::argument::column_names;

pub mod list;
pub mod dict;
pub mod re;

pub fn set_item(mut context: ExecutionContext) -> CrushResult<()> {
    three_arguments(&context.arguments)?;
    let mut container = None;
    let mut key = None;
    let mut value = None;

    for arg in context.arguments.drain(..) {
        match (arg.name.as_deref(), arg.value) {
            (Some("container"), c) => container = Some(c),
            (Some("key"), k) => key = Some(k),
            (Some("value"), l) => value = Some(l),
            _ => return argument_error("Unexpected argument"),
        }
    }

    match (container, key, value) {
        (Some(Value::List(l)), Some(Value::Integer(i)), Some(v)) => l.set(i as usize, v),
        (Some(Value::Dict(d)), Some(k), Some(v)) => d.insert(k, v),
        _ => argument_error("Missing arguments"),
    }
}

fn materialize(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.input.recv()?.materialize())
}

fn r#struct(mut context: ExecutionContext) -> CrushResult<()> {
    let mut names = column_names(&context.arguments);

    let arr: Vec<(Box<str>, Value)> =
        names.drain(..)
            .zip(context.arguments)
            .map(|(name, arg)| (name, arg.value))
            .collect::<Vec<(Box<str>, Value)>>();
    context.output.send(
        Value::Struct(Struct::new(arr)))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("data")?;
    root.r#use(&env);

    env.declare("struct", Value::Command(SimpleCommand::new(r#struct, false).boxed()))?;
    env.declare("materialize", Value::Command(SimpleCommand::new(materialize, true).boxed()))?;

    list::declare(&env)?;
    dict::declare(&env)?;
    re::declare(&env)?;
    env.readonly();

    Ok(())
}
