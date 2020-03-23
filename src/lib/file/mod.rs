use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, error, to_crush_error};
use crate::lang::{value::Value};
use crate::lang::command::{ExecutionContext, CrushCommand};
use crate::util::file::{home, cwd};
use std::path::Path;
use lazy_static::lazy_static;
use std::collections::HashMap;

mod find;
mod stat;

lazy_static! {
    pub static ref FILE_METHODS: HashMap<Box<str>, Box<dyn CrushCommand + Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand + Send + Sync>> = HashMap::new();
        res.insert(Box::from("stat"), CrushCommand::command(stat::perform, true));
        res
    };
}

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
    env.declare("ls", Value::Command(CrushCommand::command(find::perform_ls, true)))?;
    env.declare("find", Value::Command(CrushCommand::command(find::perform_find, true)))?;
    env.declare("stat", Value::Command(CrushCommand::command(stat::perform, true)))?;
    env.declare("cd", Value::Command(CrushCommand::command(cd, true)))?;
    env.declare("pwd", Value::Command(CrushCommand::command(pwd, false)))?;
    env.readonly();
    Ok(())
}
