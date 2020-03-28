use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error, to_crush_error};
use crate::lang::{value::Value, list::List, value::ValueType, command::ExecutionContext, binary::BinaryReader};
use std::env;

mod r#if;
mod r#while;
mod r#loop;
mod r#for;

use std::path::Path;
use crate::lang::command::CrushCommand;

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
    to_crush_error(env::var("PATH").map(|v| {
        let mut dirs: Vec<Value> = v
            .split(':')
            .map(|s| Value::File(Box::from(Path::new(s))))
            .collect();
        path.append(&mut dirs);
    }))?;
    env.declare("cmd_path", Value::List(path))?;

    let if_help = r#"if condition:bool if-clause:command [else-clause:command]

    Conditionally execute a command once.

    If the condition is true, the if-clause is executed. Otherwise, the else-clause
    (if specified) is executed.

    Example:

    if (./some_file:stat):is_file {echo "It's a file!"} {echo "It's not a file!"}"#;

    let while_help = r#"while condition:command body:command

    Repeatedly execute the body for as long the condition is met, or until the
    break command is called.

    In every pass of the loop, the condition is executed. If it returns false,
    the loop terminates. If it returns true, the body is executed and the loop
    continues.

    Example:

    while {not (./some_file:stat):is_file} {echo "hello"}"#;

    let LOOP_HELP = r#"loop body:command

    Repeatedly execute the body until the break command is called.
    "#;
    let FOR_HELP = r#"for [name=]iterable:(table_stream|table|dict|list) body:command

    "#;

    env.declare("if", Value::Command(CrushCommand::condition(r#if::perform, if_help)))?;
    env.declare("while", Value::Command(CrushCommand::condition(r#while::perform, while_help)))?;
    env.declare("loop", Value::Command(CrushCommand::condition(r#loop::perform, LOOP_HELP)))?;
    env.declare("for", Value::Command(CrushCommand::condition(r#for::perform, FOR_HELP)))?;
    env.declare("break", Value::Command(CrushCommand::command(r#break, false)))?;
    env.declare("continue", Value::Command(CrushCommand::command(r#continue, false)))?;
    env.declare("cmd", Value::Command(CrushCommand::command(cmd, true)))?;
    env.readonly();

    Ok(())
}
