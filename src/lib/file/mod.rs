use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, error, to_crush_error};
use crate::lang::{value::Value, command::SimpleCommand};
use crate::lang::command::{ExecutionContext, CrushCommand};
use crate::util::file::{home, cwd};
use std::path::Path;
use lazy_static::lazy_static;
use std::collections::HashMap;

mod find;
mod stat;

pub fn cd(context: ExecutionContext) -> CrushResult<()> {
    let dir = match context.arguments.len() {
        0 => home(),
        1 => {
            let dir = &context.arguments[0];
            match &dir.value {
                Value::String(val) => Ok(Box::from(Path::new(val.as_ref()))),
                Value::File(val) => Ok(val.clone()),
                Value::Glob(val) => val.glob_to_single_file(&cwd()?),
                _ => error(format!("Wrong parameter type, expected text or file, found {}", &dir.value.value_type().to_string()).as_str())
            }
        }
        _ => error("Wrong number of arguments")
    }?;
    to_crush_error(std::env::set_current_dir(dir))
}

pub fn pwd(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::File(cwd()?))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("file")?;
    root.r#use(&env);
    env.declare("ls", Value::Command(SimpleCommand::new(find::perform_ls, true).boxed()))?;
    env.declare("find", Value::Command(SimpleCommand::new(find::perform_find, true).boxed()))?;
    env.declare("stat", Value::Command(SimpleCommand::new(stat::perform, true).boxed()))?;
    env.declare("cd", Value::Command(SimpleCommand::new(cd, true).boxed()))?;
    env.declare("pwd", Value::Command(SimpleCommand::new(pwd, false).boxed()))?;
    env.readonly();
    Ok(())
}
