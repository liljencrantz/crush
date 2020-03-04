use crate::scope::Scope;
use crate::errors::CrushResult;
use crate::lang::{Value, SimpleCommand};

mod set;
mod r#let;
mod unset;
mod env;
mod r#use;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("var")?;
    root.uses(&env);
    env.declare_str("let", Value::Command(SimpleCommand::new(r#let::perform, false)))?;
    env.declare_str("set", Value::Command(SimpleCommand::new(set::perform, false)))?;
    env.declare_str("unset", Value::Command(SimpleCommand::new(unset::perform, false)))?;
    env.declare_str("env", Value::Command(SimpleCommand::new(env::perform, false)))?;
    env.declare_str("use", Value::Command(SimpleCommand::new(r#use::perform, false)))?;
    env.readonly();
    Ok(())
}
