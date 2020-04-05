use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error, to_crush_error};
use crate::lang::{value::Value, list::List, value::ValueType, execution_context::ExecutionContext, binary::BinaryReader};
use std::env;

mod r#if;
mod r#while;
mod r#loop;
mod r#for;

use std::path::Path;
use crate::lang::command::CrushCommand;
use crate::lang::printer::printer;

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
        printer().handle_error(path.append(&mut dirs));
    }))?;
    env.declare("cmd_path", Value::List(path))?;

    env.declare("if", Value::Command(CrushCommand::condition(
        r#if::perform,
        "if condition:bool if-clause:command [else-clause:command]",
        "Conditionally execute a command once.",
        Some(r#"    If the condition is true, the if-clause is executed. Otherwise, the else-clause
    (if specified) is executed.

    Example:

    if (./some_file:stat):is_file {echo "It's a file!"} {echo "It's not a file!"}"#))))?;

    env.declare("while", Value::Command(CrushCommand::condition(
        r#while::perform,
        "while condition:command body:command",
        "Repeatedly execute the body for as long the condition is met",
        Some(r#"    In every pass of the loop, the condition is executed. If it returns false,
    the loop terminates. If it returns true, the body is executed and the loop
    continues.

    Example:

    while {not (./some_file:stat):is_file} {echo "hello"}"#))))?;

    env.declare("loop", Value::Command(CrushCommand::condition(
        r#loop::perform,
        "loop body:command",
        "Repeatedly execute the body until the break command is called.",
        Some(r#"    Example:
    loop {
        if (i_am_tired) {
            break
        }
        echo "Working"
    }"#))))?;

    env.declare("for", Value::Command(CrushCommand::condition(
        r#for::perform,
        "for [name=]iterable:(table_stream|table|dict|list) body:command",
        "Execute body once for every element in iterable.",
        Some(r#"    Example:

    for (seq) {
        echo ("Lap {}":format value)
    }"#))))?;


    env.declare_command(
        "break",r#break, false,
        "break", "Stop execution of a loop", None)?;
    env.declare_command(
        "continue",r#continue, false,
        "continue", "Skip execution of the current iteration of a loop", None)?;
    env.declare_command(
        "cmd",cmd, true,
        "cmd external_command:(file|string) @arguments:any", "Execute external commands", None)?;
    env.readonly();

    Ok(())
}
