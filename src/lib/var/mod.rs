use crate::scope::Scope;
use crate::errors::CrushResult;
use crate::data::{Value, Command};

mod set;
mod r#let;
mod unset;
mod env;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("var")?;
    root.uses(&env);
    env.declare_str("let", Value::Command(Command::new(r#let::perform)))?;
    env.declare_str("set", Value::Command(Command::new(set::perform)))?;
    env.declare_str("unset", Value::Command(Command::new(unset::perform)))?;
    env.declare_str("env", Value::Command(Command::new(env::perform)))?;
    env.readonly();
    Ok(())
}
