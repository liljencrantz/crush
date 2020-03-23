use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, r#struct::Struct};
use crate::lang::command::{ExecutionContext, CrushCommand, This};
use crate::lang::argument::column_names;
use crate::lang::command::ArgumentVector;
pub mod list;
pub mod dict;
pub mod re;

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

pub fn setattr(mut context: ExecutionContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    let name = context.arguments.string(0)?;
    let value = context.arguments.value(1)?;
    this.set(&name, value);
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("data")?;
    root.r#use(&env);

    env.declare("struct", Value::Command(CrushCommand::command(r#struct, false)))?;
    env.declare("materialize", Value::Command(CrushCommand::command(materialize, true)))?;

    list::declare(&env)?;
    dict::declare(&env)?;
    re::declare(&env)?;
    env.readonly();

    Ok(())
}
