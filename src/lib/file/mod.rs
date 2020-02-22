use crate::scope::Scope;
use crate::errors::CrushResult;
use crate::lang::{Value, Command};

mod find;
mod stat;
mod cd;
mod pwd;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("file")?;
    root.uses(&env);
    env.declare_str("ls", Value::Command(Command::new(find::perform_ls)))?;
    env.declare_str("find", Value::Command(Command::new(find::perform_find)))?;
    env.declare_str("stat", Value::Command(Command::new(stat::perform)))?;
    env.declare_str("cd", Value::Command(Command::new(cd::perform)))?;
    env.declare_str("pwd", Value::Command(Command::new(pwd::perform)))?;
    env.readonly();
    Ok(())
}
