use crate::lang::errors::{argument_error_legacy, to_crush_error, CrushResult, mandate, data_error};
use crate::lang::data::scope::Scope;
use crate::lang::{
    data::binary::BinaryReader, execution_context::CommandContext, data::list::List, value::Value,
    value::ValueType,
};
use signature::signature;
use std::env;

use crate::lang::command::OutputType::Known;
use chrono::Duration;
use std::path::PathBuf;
use crate::lang::data::table::{ColumnType, Row};
use std::io::Stdin;
use std::process::Stdio;

mod r#for;
mod r#if;
mod r#loop;
mod sudo;
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

            if use_tty {
                cmd
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit());
            }
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
                to_crush_error(to_crush_error(cmd.spawn())?.wait())?;
                Ok(())
            } else {
                let output = to_crush_error(cmd.output())?;
                let errors = String::from_utf8_lossy(&output.stderr);
                for e in errors.split('\n') {
                    let err = e.trim();
                    if !err.is_empty() {
                        context.global_state.printer().error(err);
                    }
                }
                context
                    .output
                    .send(Value::BinaryInputStream(BinaryReader::vec(&output.stdout)))
            }
        }
        _ => argument_error_legacy("Not a valid command"),
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
    let mut result_stream = mandate(context.input.recv()?.stream(), "Invalid input")?;
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
            r#while::While::declare(env)?;
            r#loop::Loop::declare(env)?;
            sudo::Sudo::declare(env)?;

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
                "cmd external_command:(file|string) @arguments:any",
                "Execute external commands",
                None,
                Known(ValueType::BinaryInputStream),
                vec![],
            )?;
            Break::declare(env)?;
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
