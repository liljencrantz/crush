use crate::scope::Scope;
use crate::errors::CrushResult;
use crate::data::{Value, Command};

mod echo;
mod lines;
mod csv;
mod json;
mod cat;
mod http;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("io")?;
    root.uses(&env);
    env.declare_str("cat", Value::Command(Command::new(cat::perform)))?;
    env.declare_str("http", Value::Command(Command::new(http::perform)))?;
    env.declare_str("lines", Value::Command(Command::new(lines::perform)))?;
    env.declare_str("csv", Value::Command(Command::new(csv::perform)))?;
    env.declare_str("json", Value::Command(Command::new(json::perform)))?;
    env.declare_str("echo", Value::Command(Command::new(echo::perform)))?;
    env.readonly();

    Ok(())
}
