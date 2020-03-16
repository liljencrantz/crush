use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error, to_crush_error};
use crate::lang::{value::Value, command::SimpleCommand, list::List, value::ValueType, command::ConditionCommand, command::ExecutionContext, binary::BinaryReader};
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
    root.r#use(&env);

    let path = List::new(ValueType::File, vec![]);
    env::var("PATH").map(|v| {
        let mut dirs: Vec<Value> = v
            .split(':')
            .map(|s| Value::File(Box::from(Path::new(s))))
            .collect();
        path.append(&mut dirs);
    });
    env.declare("cmd_path", Value::List(path));

    env.declare("if", Value::ConditionCommand(ConditionCommand::new(r#if::perform)))?;
    env.declare("while", Value::ConditionCommand(ConditionCommand::new(r#while::perform)))?;
    env.declare("loop", Value::ConditionCommand(ConditionCommand::new(r#loop::perform)))?;
    env.declare("for", Value::ConditionCommand(ConditionCommand::new(r#for::perform)))?;
    env.declare("break", Value::Command(SimpleCommand::new(r#break, false)))?;
    env.declare("continue", Value::Command(SimpleCommand::new(r#continue, false)))?;
    env.declare("cmd", Value::Command(SimpleCommand::new(cmd, true)))?;
    env.readonly();

    Ok(())
}
