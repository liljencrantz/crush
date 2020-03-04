use crate::scope::Scope;
use crate::errors::CrushResult;
use crate::lang::{Value, SimpleCommand, List, ValueType, ConditionCommand};
use std::env;

mod r#if;
mod r#while;
mod r#loop;
mod r#for;
mod r#break;
mod r#continue;
mod cmd;

pub use cmd::cmd;
use std::path::Path;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("control")?;
    root.uses(&env);

    let path = List::new(ValueType::File, vec![]);
    env::var("PATH").map(|v| {
        let mut dirs: Vec<Value> = v
            .split(':')
            .map(|s| Value::File(Box::from(Path::new(s))))
            .collect();
        path.append(&mut dirs);
    });
    env.declare_str("cmd_path", Value::List(path));

    env.declare_str("if", Value::ConditionCommand(ConditionCommand::new(r#if::perform)))?;
    env.declare_str("while", Value::ConditionCommand(ConditionCommand::new(r#while::perform)))?;
    env.declare_str("loop", Value::ConditionCommand(ConditionCommand::new(r#loop::perform)))?;
    env.declare_str("for", Value::ConditionCommand(ConditionCommand::new(r#for::perform)))?;
    env.declare_str("break", Value::Command(SimpleCommand::new(r#break::perform, false)))?;
    env.declare_str("continue", Value::Command(SimpleCommand::new(r#continue::perform, false)))?;
    env.declare_str("cmd", Value::Command(SimpleCommand::new(cmd::cmd, true)))?;
    env.readonly();

    Ok(())
}
