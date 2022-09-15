use crate::lang::errors::{argument_error_legacy, CrushResult, data_error, mandate, to_crush_error};
use crate::lang::state::scope::Scope;
use crate::lang::{
    data::binary::BinaryReader, data::list::List, value::Value,
    value::ValueType,
};
use signature::signature;
use std::env;

use crate::lang::command::OutputType::Known;
use chrono::Duration;
use std::path::PathBuf;
use crate::lang::data::table::{ColumnType, Row};
use std::io::{Read, Write};
use std::process::Stdio;
use std::borrow::BorrowMut;
use crate::lang::value::Value::BinaryInputStream;
use os_pipe::PipeReader;
use crate::lang::state::contexts::CommandContext;

mod r#for;
mod r#if;
mod r#loop;
mod timer;
mod r#while;

#[signature(
r#break,
can_block = false,
short = "Stop execution of a loop.",
output = Known(ValueType::Empty))]
struct Break {}

fn r#break(context: CommandContext) -> CrushResult<()> {
    context.scope.do_break()?;
    context.output.empty()
}

#[signature(
r#continue,
can_block = false,
short = "Skip execution of the current iteration of a loop.",
output = Known(ValueType::Empty))]
struct Continue {}

fn r#continue(context: CommandContext) -> CrushResult<()> {
    context.scope.do_continue()?;
    context.output.empty()
}

fn cmd(mut context: CommandContext) -> CrushResult<()> {
    if context.arguments.is_empty() {
        return argument_error_legacy("No command given");
    }
    match context.arguments.remove(0).value {
        Value::File(f) => {
            let use_tty = !context.input.is_pipeline() && !context.output.is_pipeline();
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

            if use_tty {
                cmd
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit());

                to_crush_error(to_crush_error(cmd.spawn())?.wait())?;
                Ok(())
            } else {
                let input = context.input.recv()?;

                let (stdout_reader, stdout_writer) = os_pipe::pipe().unwrap();
                let (mut stderr_reader, stderr_writer) = os_pipe::pipe().unwrap();

                cmd.stdin(Stdio::piped());
                cmd.stdout(stdout_writer);
                cmd.stderr(stderr_writer);

                let mut child = to_crush_error(cmd.spawn())?;
                let mut stdin = mandate(child.stdin.take(), "Expected stdin stream")?;

                match input {
                    Value::Empty() => {
                        drop(stdin);
                    }
                    Value::Binary(v) => {
                        context.spawn("cmd:stdin", move || {
                            stdin.write(&v)?;
                            Ok(())
                        })?;
                    }
                    Value::BinaryInputStream(mut r) => {
                        context.spawn("cmd:stdin", move || {
                            to_crush_error(std::io::copy(r.as_mut(), stdin.borrow_mut()))?;
                            Ok(())
                        })?;
                    }
                    _ => return argument_error_legacy("Invalid inpuy: Expected binary data"),
                }

                context.output.send(BinaryInputStream(Box::from(stdout_reader)))?;
                let my_context = context.clone();
                context.spawn("cmd:stderr", move || {
                    let _ = &my_context;
                    let mut buff = Vec::new();
                    to_crush_error(stderr_reader.read_to_end(&mut buff))?;
                    let errors = to_crush_error(String::from_utf8(buff))?;
                    for e in errors.split('\n') {
                        let err = e.trim();
                        if !err.is_empty() {
                            my_context.global_state.printer().error(err);
                        }
                    }
                    Ok(())
                })?;

                child.wait()?;

                Ok(())
            }
        }
        _ => argument_error_legacy("Not a valid command"),
    }
}

impl BinaryReader for PipeReader {
    fn clone(&self) -> Box<dyn BinaryReader + Send + Sync> {
        Box::new(self.try_clone().unwrap())
    }
}

#[signature(
sleep,
can_block = true,
short = "Pause execution of commands for the specified amount of time",
long = "    Execute the specified command all specified hosts"
)]
struct Sleep {
    #[description("the time to sleep for.")]
    duration: Duration,
}

fn sleep(context: CommandContext) -> CrushResult<()> {
    let cfg = Sleep::parse(context.arguments, &context.global_state.printer())?;
    std::thread::sleep(to_crush_error(cfg.duration.to_std())?);
    context.output.send(Value::Empty())?;
    Ok(())
}

#[signature(
bg,
short = "Run a pipeline in background",
example = "pipe := ((table_input_stream value=integer):pipe)\n    _1 := (seq 100_000 | pipe:output:write | bg)\n    sum_job_id := (pipe:input | sum | bg)\n    pipe:close\n    sum_job_id | fg"
)]
struct Bg {}

fn bg(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(
        vec![ColumnType::new("value", ValueType::Any)])?;
    if let Ok(value) = context.input.recv() {
        output.send(Row::new(vec![value]))?;
    }
    Ok(())
}

#[signature(
fg,
short = "Return the output of a background pipeline",
example = "pipe := ((table_input_stream value=integer):pipe)\n    _1 := (seq 100_000 | pipe:output:write | bg)\n    sum_job_id := (pipe:input | sum | bg)\n    pipe:close\n    sum_job_id | fg"
)]
struct Fg {}

fn fg(context: CommandContext) -> CrushResult<()> {
    let mut result_stream = mandate(context.input.recv()?.stream()?, "Invalid input")?;
    let mut result: Vec<Value> = result_stream.read()?.into();
    if result.len() != 1 {
        data_error("Expected a single row, single column result")
    } else {
        context.output.send(result.remove(0))
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "control",
        "Commands for flow control, (loops, etc)",
        Box::new(move |env| {
            let path = List::new(ValueType::File, []);
            to_crush_error(env::var("PATH").map(|v| {
                let mut dirs: Vec<Value> = v
                    .split(':')
                    .map(|s| Value::File(PathBuf::from(s)))
                    .collect();
                let _ = path.append(&mut dirs);
            }))?;
            env.declare("cmd_path", Value::List(path))?;
            r#if::If::declare(env)?;
            r#while::While::declare(env)?;
            r#loop::Loop::declare(env)?;

            env.declare_condition_command(
                "for",
                r#for::r#for,
                "for [name=](table_input_stream|table|dict|list) body:command",
                "Execute body once for every element in iterable.",
                Some(
                    r#"    Example:

    for (seq 10) {
        echo ("Lap #{}":format value)
    }"#,
                ),
                vec![],
            )?;

            env.declare_command(
                "cmd",
                cmd,
                true,
                "cmd external_command:file @arguments:any",
                "Execute external commands",
                None,
                Known(ValueType::BinaryInputStream),
                vec![],
            )?;
            Break::declare(env)?;
            timer::Timer::declare(env)?;
            Continue::declare(env)?;
            Sleep::declare(env)?;
            Bg::declare(env)?;
            Fg::declare(env)?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
