use crate::lang::errors::{CrushResult, data_error, mandate};
use crate::lang::state::scope::Scope;
use crate::lang::{
    data::binary::BinaryReader, data::list::List, value::Value,
    value::ValueType,
};
use signature::signature;
use std::env;

use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
use chrono::Duration;
use std::path::PathBuf;
use crate::lang::data::table::{ColumnType, Row};
use os_pipe::PipeReader;
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;

mod cmd;
mod help;
mod r#for;
mod r#if;
mod r#loop;
mod timeit;
mod timer;
mod schedule;
mod r#while;

#[signature(
    control.r#break,
    can_block = false,
    short = "Stop execution of a loop.",
    output = Known(ValueType::Empty))]
struct Break {}

fn r#break(context: CommandContext) -> CrushResult<()> {
    context.scope.do_break()?;
    context.output.empty()
}

#[signature(
    control.r#return,
    can_block = false,
    short = "Stop execution of a closure and return a value.",
    output = Unknown)]
struct Return {
    #[description("the value to return")]
    value: Option<Value>,
}

fn r#return(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Return::parse(context.remove_arguments(), context.global_state.printer())?;
    context.scope.do_return(cfg.value)?;
    context.output.empty()
}

#[signature(
    control.r#continue,
    can_block = false,
    short = "Skip execution of the current iteration of a loop.",
    output = Known(ValueType::Empty))]
struct Continue {}

fn r#continue(context: CommandContext) -> CrushResult<()> {
    context.scope.do_continue()?;
    context.output.empty()
}

impl BinaryReader for PipeReader {
    fn clone(&self) -> Box<dyn BinaryReader + Send + Sync> {
        Box::new(self.try_clone().unwrap())
    }
}

#[signature(
    control.sleep,
    can_block = true,
    short = "Pause execution of commands for the specified amount of time",
)]
struct Sleep {
    #[description("the time to sleep for.")]
    duration: Duration,
}

fn sleep(context: CommandContext) -> CrushResult<()> {
    let cfg = Sleep::parse(context.arguments, &context.global_state.printer())?;
    std::thread::sleep(cfg.duration.to_std()?);
    context.output.send(Value::Empty)?;
    Ok(())
}

#[signature(
    control.bg,
    short = "Run a pipeline in background",
    example = "$pipe := $($(table_input_stream value=integer):pipe)\n    $_1 := $(seq 100_000 | pipe:output:write | bg)\n    $sum_job_id := $($pipe:input | sum | bg)\n    $pipe:close\n    $sum_job_id | fg"
)]
struct Bg {}

fn bg(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(
        &[ColumnType::new("value", ValueType::Any)])?;
    if let Ok(value) = context.input.recv() {
        output.send(Row::new(vec![value]))?;
    }
    Ok(())
}

#[signature(
    control.fg,
    short = "Return the output of a background pipeline",
    example = "$pipe := $($(table_input_stream value=integer):pipe)\n    $_1 := $(seq 100_000 | pipe:output:write | bg)\n    $sum_job_id := $($pipe:input | sum | bg)\n    $pipe:close\n    $sum_job_id | fg"
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

#[signature(
    control.source,
    short = "Evaluate files into current crush session",
    example = "source *.crush"
)]
struct Source {
    #[unnamed()]
    #[description("the files to source")]
    files: Files,
}

fn source(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Source = Source::parse(context.remove_arguments(), &context.global_state.printer())?;
    for file in Vec::<PathBuf>::from(cfg.files) {
        crate::execute::file(&context.scope, &file, &context.output, &context.global_state)?;
    }
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "control",
        "Commands for flow control, (loops, etc)",
        Box::new(move |env| {
            let path = List::new(ValueType::File, []);
            env::var("PATH").map(|v| {
                let mut dirs: Vec<Value> = v
                    .split(':')
                    .map(|s| Value::from(PathBuf::from(s)))
                    .collect();
                let _ = path.append(&mut dirs);
            })?;
            env.declare("cmd_path", path.into())?;
            r#if::If::declare(env)?;
            r#while::While::declare(env)?;
            r#loop::Loop::declare(env)?;
            r#for::For::declare(env)?;
            cmd::Cmd::declare(env)?;
            Break::declare(env)?;
            Return::declare(env)?;
            timeit::TimeIt::declare(env)?;
            timer::Timer::declare(env)?;
            schedule::Schedule::declare(env)?;
            Continue::declare(env)?;
            Sleep::declare(env)?;
            Bg::declare(env)?;
            Fg::declare(env)?;
            help::HelpSignature::declare(env)?;
            Source::declare(env)?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
