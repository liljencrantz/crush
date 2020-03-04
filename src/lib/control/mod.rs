use crate::scope::Scope;
use crate::errors::{CrushResult, argument_error, to_crush_error};
use crate::lang::{Value, SimpleCommand, List, ValueType, ConditionCommand, ExecutionContext, BinaryReader};
use std::env;

mod r#if;
mod r#while;
mod r#loop;
mod r#for;

use std::path::Path;

pub fn r#break(context: ExecutionContext) -> CrushResult<()> {
    context.env.do_break();
    Ok(())
}

pub fn r#continue(context: ExecutionContext) -> CrushResult<()> {
    context.env.do_continue();
    Ok(())
}

pub fn cmd(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() == 0 {
        return argument_error("No command given");
    }
    match context.arguments.remove(0).value {
        Value::File(f) => {
            let mut cmd = std::process::Command::new(f.as_os_str());
            for a in context.arguments.drain(..) {
                cmd.arg(a.value.to_string());
            }
            let output = to_crush_error(cmd.output())?;
            context.output.send(
                Value::BinaryStream(
                    BinaryReader::vec(&output.stdout)))
        }
        _ => argument_error("Not a valid command")
    }
}

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
    env.declare_str("break", Value::Command(SimpleCommand::new(r#break, false)))?;
    env.declare_str("continue", Value::Command(SimpleCommand::new(r#continue, false)))?;
    env.declare_str("cmd", Value::Command(SimpleCommand::new(cmd, true)))?;
    env.readonly();

    Ok(())
}
