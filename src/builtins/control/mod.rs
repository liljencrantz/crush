use crate::lang::errors::{CrushResult, data_error, argument_error_legacy};
use crate::lang::state::scope::Scope;
use crate::lang::{data::binary::BinaryReader, data::list::List, value::Value, value::ValueType};
use signature::signature;
use std::env;

use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use chrono::Duration;
use os_pipe::PipeReader;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::AtomicU64;
use ordered_map::OrderedMap;
use crate::lang::any_str::AnyStr;
use crate::lang::command::{Command, CrushCommand};
use crate::lang::data::r#struct::Struct;
use crate::lang::pipe::{TableInputStream, ValueReceiver};
use crate::lang::state::this::This;

mod cmd;
mod r#for;
mod help;
mod r#if;
mod r#loop;
mod schedule;
mod timeit;
mod timer;
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
    example = "sleep $(duration:of seconds=10)",
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
    long = "Append the `bg` builtin to the end of a pipeline to run it in the background.",
    long = "",
    long = "The `bg` builtin will read the result from a pipeline and insert it into a table output stream.",
    long = "Because this stream is immediately returned, execution will continue and the pipeline will run",
    long = "in the background.",
    long = "",
    long = "To get the result of the pipeline, use the `fg` builtin.",
    example = "# Create a pipe",
    example = "$pipe := $($(table_input_stream value=$integer):pipe)",
    example = "# Create a job that writes 100_000 integers to the pipe and put this job in the background",
    example = "$_1 := $(seq 100_000 | pipe:write | bg)",
    example = "# Create a second job that reads from the pipe and sums all the integers and put this job in the background",
    example = "$sum_job_handle := $(pipe:read | sum | bg)",
    example = "# Close the pipe so that the second job can finish",
    example = "pipe:close",
    example = "# Put the sum job in the foreground",
    example = "sum_job_handle | fg",
)]
struct Bg {}

#[derive(Clone)]
struct BackgroundJob {
    id: u64,
    value: ValueReceiver,
}

static BG_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

fn background_jobs() -> &'static Mutex<Vec<BackgroundJob>> {
    static CELL: OnceLock<Mutex<Vec<BackgroundJob>>> = OnceLock::new();
    CELL.get_or_init(|| {
        Mutex::new(Vec::new())
    })
}

fn remove_job(id: u64) -> Option<ValueReceiver> {
    let mut jobs = background_jobs().lock().unwrap();
    let mut matching = jobs.extract_if(.., |job| job.id == id).collect::<Vec<_>>();
    matching.pop().map(|job| job.value)
}

fn remove_last_job() -> Option<ValueReceiver> {
    let mut jobs = background_jobs().lock().unwrap();
    jobs.pop().map(|job| job.value)
}

fn add_job(value: ValueReceiver) -> u64 {
    let mut jobs = background_jobs().lock().unwrap();
    let id = BG_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Acquire);
    jobs.push(BackgroundJob {
        id,
        value,   
    });
    id
}

fn bg(context: CommandContext) -> CrushResult<()> {
    let id = add_job(context.input.clone());

    context.output.send(Value::Struct(
        Struct::new(vec![
            ("id", Value::from(id)),
        ], None)
    ))
}

#[signature(
    control.fg,
    short = "Return the output of a background pipeline",
    long = "The bg builtin will read the result from a pipeline and insert it into a table output stream.",
    long = "Because this stream is immediately returned, execution will continue and the pipeline will run",
    long = "in the background.",
    long = "",
    long = "To get the result of the pipeline, use the fg builtin.",
    example = "# Create a pipe",
    example = "$pipe := $($(table_input_stream value=$integer):pipe)",
    example = "# Create a job that writes 100_000 integers to the pipe and put this job in the background",
    example = "$_1 := $(seq 100_000 | pipe:write | bg)",
    example = "# Create a second job that reads from the pipe and sums all the integers and put this job in the background",
    example = "$sum_job_handle := $(pipe:read | sum | bg)",
    example = "# Close the pipe so that the second job can finish",
    example = "pipe:close",
    example = "# Put the sum job in the foreground",
    example = "sum_job_handle | fg",
)]
struct Fg {
    job: Option<Struct>,
}

fn fg(context: CommandContext) -> CrushResult<()> {
    let cfg = Fg::parse(context.arguments, &context.global_state.printer())?;
    match cfg.job {
        None => match remove_last_job() {
            None => {
                context.output.send(Value::Empty)
            }
            Some(v) => {
                context.output.send(v.recv()?)
            }
        }

        Some(handle) => {
            match handle.get("id") {
                Some(Value::Integer(id)) => {
                    match remove_job(id as u64) {
                        None => {
                            context.output.send(Value::Empty)
                        }
                        Some(v) => {
                            context.output.send(v.recv()?)
                        }
                    }
                }
                _ => argument_error_legacy("Expected an output stream"),
            }
        }
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
        crate::execute::file(
            &context.scope,
            &file,
            &context.output,
            &context.global_state,
        )?;
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
