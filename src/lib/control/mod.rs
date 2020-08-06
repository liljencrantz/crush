use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error, to_crush_error};
use crate::lang::{value::Value, list::List, value::ValueType, execution_context::ExecutionContext, binary::BinaryReader};
use std::env;
use signature::signature;

mod r#if;
mod r#while;
mod r#loop;
mod r#for;

use std::path::PathBuf;
use chrono::Duration;
use crate::lang::argument::ArgumentHandler;
use crate::lang::command::OutputType::Known;

pub fn r#break(context: ExecutionContext) -> CrushResult<()> {
    context.env.do_break()?;
    context.output.empty()
}

pub fn r#continue(context: ExecutionContext) -> CrushResult<()> {
    context.env.do_continue()?;
    context.output.empty()
}

pub fn cmd(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.is_empty() {
        return argument_error("No command given");
    }
    match context.arguments.remove(0).value {
        Value::File(f) => {
            let mut cmd = std::process::Command::new(f.as_os_str());
            for a in context.arguments.drain(..) {
                match a.argument_type {
                    None => {
                        cmd.arg(a.value.to_string());
                    }
                    Some(name) => {
                        if name.len() == 1 {
                            cmd.arg(format!("-{}", name));
                        } else {
                            cmd.arg(format!("--{}", name));
                        }
                        match a.value {
                            Value::Bool(true) => {}
                            _ => {
                                cmd.arg(a.value.to_string());
                            }
                        }
                    }
                }
            }
            let output = to_crush_error(cmd.output())?;
            let errors = String::from_utf8_lossy(&output.stderr);
            for e in errors.split('\n') {
                let err = e.trim();
                if !err.is_empty() {
                    context.printer.error(err);
                }
            }
            context.output.send(
                Value::BinaryStream(
                    BinaryReader::vec(&output.stdout)))
        }
        _ => argument_error("Not a valid command")
    }
}

#[signature(
sleep,
can_block = true,
short = "Pause execution of commands for the specified amount of time",
long = "    Execute the specified command all specified hosts")]
struct Sleep {
    #[description("the time to sleep for.")]
    duration: Duration,
}

pub fn sleep(context: ExecutionContext) -> CrushResult<()> {
    let cfg = Sleep::parse(context.arguments, &context.printer)?;
    std::thread::sleep(to_crush_error(cfg.duration.to_std())?);
    context.output.send(Value::Empty())?;
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_lazy_namespace(
        "control",
        Box::new(move |env| {
            let path = List::new(ValueType::File, vec![]);
            to_crush_error(env::var("PATH").map(|v| {
                let mut dirs: Vec<Value> = v
                    .split(':')
                    .map(|s| Value::File(PathBuf::from(s)))
                    .collect();
                let _ = path.append(&mut dirs);
            }))?;
            env.declare("cmd_path", Value::List(path))?;
            r#if::If::declare(env)?;

            env.declare_condition_command(
                "while",
                r#while::r#while,
                "while condition:command [body:command]",
                "Repeatedly execute the body for as long the condition is met",
                Some(r#"    In every pass of the loop, the condition is executed. If it returns false,
    the loop terminates. If it returns true, the body is executed and the loop
    continues.

    Example:

    while {not (./some_file:stat):is_file} {echo "hello"}

    The loop body is optional. If not specified, the condition is executed until it returns false.
    This effectively means that the condition becomes the body, and the loop break check comes at
    the end of the loop."#))?;

            r#loop::Loop::declare(env)?;

            env.declare_condition_command(
                "for",
                r#for::r#for,
                "for [name=]iterable:(table_stream|table|dict|list) body:command",
                "Execute body once for every element in iterable.",
                Some(r#"    Example:

    for (seq 10) {
        echo ("Lap #{}":format value)
    }"#))?;


            env.declare_command(
                "break", r#break, false,
                "break", "Stop execution of a loop", None, Known(ValueType::Empty))?;
            env.declare_command(
                "continue", r#continue, false,
                "continue",
                "Skip execution of the current iteration of a loop",
                None, Known(ValueType::Empty))?;
            env.declare_command(
                "cmd", cmd, true,
                "cmd external_command:(file|string) @arguments:any",
                "Execute external commands",
                None, Known(ValueType::BinaryStream))?;
            Sleep::declare(env)?;
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
