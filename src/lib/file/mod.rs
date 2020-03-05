use crate::lang::scope::Scope;
use crate::errors::CrushResult;
use crate::lang::{value::Value, command::SimpleCommand};

mod find;
mod stat;
mod cd;
mod pwd;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("file")?;
    root.uses(&env);
    env.declare_str("ls", Value::Command(SimpleCommand::new(find::perform_ls, true)))?;
    env.declare_str("find", Value::Command(SimpleCommand::new(find::perform_find, true)))?;
    env.declare_str("stat", Value::Command(SimpleCommand::new(stat::perform, true)))?;
    env.declare_str("cd", Value::Command(SimpleCommand::new(cd::perform, true)))?;
    env.declare_str("pwd", Value::Command(SimpleCommand::new(pwd::perform, false)))?;
    env.readonly();
    Ok(())
}
