use crate::namespace::Namespace;
use crate::errors::CrushResult;
use crate::data::{Value, Command};

mod r#if;
mod r#while;
mod r#for;
mod r#break;
mod r#continue;

pub fn declare(root: &Namespace) -> CrushResult<()> {
    let env = root.create_namespace("control")?;
    root.uses(&env);

    root.declare_str("if", Value::Command(Command::new(r#if::perform)))?;
    root.declare_str("while", Value::Command(Command::new(r#while::perform)))?;
    root.declare_str("for", Value::Command(Command::new(r#for::perform)))?;
    env.declare_str("break", Value::Command(Command::new(r#break::perform)))?;
    env.declare_str("continue", Value::Command(Command::new(r#continue::perform)))?;

    Ok(())
}
