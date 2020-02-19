use crate::namespace::Namespace;
use crate::errors::CrushResult;
use crate::data::{Value, Command};

mod r#struct;
mod val;
mod materialize;
mod list;
mod dict;

pub fn declare(root: &Namespace) -> CrushResult<()> {
    let env = root.create_namespace("data")?;
    root.uses(&env);

    env.declare_str("struct", Value::Command(Command::new(r#struct::perform)))?;
    env.declare_str("val", Value::Command(Command::new(val::perform)))?;
    env.declare_str("materialize", Value::Command(Command::new(materialize::perform)))?;

    list::declare(&env)?;
    dict::declare(&env)?;

    Ok(())
}
