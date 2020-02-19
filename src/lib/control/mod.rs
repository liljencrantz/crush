use crate::namespace::Namespace;
use crate::errors::CrushResult;
use crate::data::{Value, Command};

mod r#for;
mod r#if;

pub fn declare(root: &Namespace) -> CrushResult<()> {
    let env = root.create_namespace("control")?;
    root.uses(&env);

    root.declare_str("if", Value::Command(Command::new(r#if::perform)))?;
    root.declare_str("for", Value::Command(Command::new(r#for::perform)))?;

    Ok(())
}
