use crate::scope::Scope;
use crate::errors::CrushResult;
use crate::data::{Value, Command};

mod r#if;
mod r#while;
mod r#for;
mod r#break;
mod r#continue;
mod r#cmd;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("control")?;
    root.uses(&env);

    env.declare_str("if", Value::Command(Command::new(r#if::perform)))?;
    env.declare_str("while", Value::Command(Command::new(r#while::perform)))?;
    env.declare_str("for", Value::Command(Command::new(r#for::perform)))?;
    env.declare_str("break", Value::Command(Command::new(r#break::perform)))?;
    env.declare_str("continue", Value::Command(Command::new(r#continue::perform)))?;
    env.declare_str("cmd", Value::Command(Command::new(r#cmd::cmd)))?;
    env.readonly();

    Ok(())
}
