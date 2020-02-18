use crate::namepspace::Namespace;
use crate::errors::CrushResult;
use crate::data::{Value, Command};

mod lines;
mod csv;
mod json;
mod cat;
mod http;

pub fn declare(root: &Namespace) -> CrushResult<()> {
    let env = root.create_namespace("io")?;
    root.uses(&env);

    root.declare_str("cat", Value::Command(Command::new(cat::perform)))?;
    root.declare_str("http", Value::Command(Command::new(http::perform)))?;
    root.declare_str("lines", Value::Command(Command::new(lines::perform)))?;
    root.declare_str("csv", Value::Command(Command::new(csv::perform)))?;
    root.declare_str("json", Value::Command(Command::new(json::perform)))?;

    Ok(())
}
