use crate::lang::scope::Scope;
use crate::errors::CrushResult;
use crate::lang::{value::Value, command::SimpleCommand, r#struct::Struct};
use crate::lang::command::ExecutionContext;

mod list;
mod dict;
mod re;

fn materialize(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.input.recv()?.materialize())
}

fn r#struct(mut context: ExecutionContext) -> CrushResult<()> {
    let arr: Vec<(Box<str>, Value)> = context.arguments.drain(..)
        .map(|v| (Box::from(v.name.unwrap()), v.value))
        .collect::<Vec<(Box<str>, Value)>>();
    context.output.send(
        Value::Struct(Struct::new(arr)))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("data")?;
    root.uses(&env);

    env.declare_str("struct", Value::Command(SimpleCommand::new(r#struct, false)))?;
    env.declare_str("materialize", Value::Command(SimpleCommand::new(materialize, true)))?;

    list::declare(&env)?;
    dict::declare(&env)?;
    re::declare(&env)?;
    env.readonly();

    Ok(())
}
