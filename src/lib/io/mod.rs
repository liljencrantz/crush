use crate::scope::Scope;
use crate::errors::CrushResult;
use crate::lang::{Value, SimpleCommand};

mod echo;
mod lines;
mod csv;
mod json;
mod cat;
mod http;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("io")?;
    root.uses(&env);
    env.declare_str("cat", Value::Command(SimpleCommand::new(cat::perform)))?;
    env.declare_str("http", Value::Command(SimpleCommand::new(http::perform)))?;
    env.declare_str("lines", Value::Command(SimpleCommand::new(lines::perform)))?;
    env.declare_str("csv", Value::Command(SimpleCommand::new(csv::perform)))?;
    env.declare_str("json", Value::Command(SimpleCommand::new(json::perform)))?;
    env.declare_str("echo", Value::Command(SimpleCommand::new(echo::perform)))?;
    env.readonly();

    Ok(())
}
