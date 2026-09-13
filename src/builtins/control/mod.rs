use crate::lang::ast::lexer::LanguageMode;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
use crate::lang::command_invocation::resolve_external_command;
use crate::lang::errors::{CrushResult, command_error, terminate};
use crate::lang::job_control::{ChannelBasedController, StreamControlMessage};
use crate::lang::signature::binary_input::BinaryInput;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::handles::JobType::Background;
use crate::lang::state::scope::Scope;
use crate::lang::{data::binary::BinaryReader, value::Value, value::ValueType};
use crate::util::file::cwd;
use crate::util::regex::RegexFileMatcher;
use chrono::Duration;
use crossbeam::channel::bounded;
use os_pipe::PipeReader;
use signature::signature;
use std::io::Read;

mod cmd;
mod r#for;
mod help;
mod r#if;
mod r#loop;
mod r#match;
mod schedule;
mod timeit;
mod r#try;
mod timer;
mod r#while;

#[signature(
    control.r#break,
    can_block = false,
    short = "Break execution of a loop.",
    output = Known(ValueType::Empty))]
struct Break {}

fn r#break(context: CommandContext) -> CrushResult<()> {
    context.scope.do_break()?;
    context.output.empty()
}

#[signature(
    control.r#return,
    can_block = false,
    short = "break execution of a closure command and optionally return a value.",
    long = "The `return` command can only be used when inside of a closure command. Closure blocks can not break early using the `return` command. Do note that you can use the `return` command inside of a closure block which is nested arbitrarily deeply inside of a closure command, which will stop execution of all inner blocks and return the closure command.",
    output = Unknown,
    example = "# Define a factorial command",
    example = "$factorial := {",
    example = "  |$number: $integer|",
    example = "  if ($number == 1) {",
    example = "    return 1",
    example = "  } else {",
    example = "    return ($number * factorial($number - 1))",
    example = "  }",
    example = "}",
    example = "# Call the command",
    example = "factorial 5"
)]
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
    short = "Break execution of the current iteration of a loop and continue to the next lap.",
    output = Known(ValueType::Empty))]
struct Continue {}

fn r#continue(context: CommandContext) -> CrushResult<()> {
    context.scope.do_continue()?;
    context.output.empty()
}

#[signature(
    control.assert,
    can_block = false,
    short = "Error out if the condition is false.",
    output = Known(ValueType::Empty),
    example = "assert (1 + 1 == 2)",
    example = "assert ($x > 0) \"x must be positive\"",
)]
struct Assert {
    #[description("the condition to check.")]
    condition: bool,
    #[description("the message to show if the condition is false.")]
    #[default("Assertion failed")]
    message: String,
}

fn assert(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Assert = Assert::parse(context.remove_arguments(), &context.global_state.printer())?;
    if cfg.condition {
        context.output.empty()
    } else {
        command_error(cfg.message)
    }
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

fn sleep(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Sleep::parse(context.remove_arguments(), &context.global_state.printer())?;

    // A plain std::thread::sleep() here would be uninterruptible: crush:terminate/
    // crush:pause reach a running command only by sending a StreamControlMessage
    // through a channel it registered for itself, so a sleep that never registers one
    // and never checks it just runs to completion regardless -- this was a real,
    // confirmed bug (see the audit). Registering a controller and polling it via
    // recv_timeout -- and, on Pause, blocking for Resume/Terminate rather than
    // continuing to count down -- exactly matches control::schedule's own internal
    // sleep helper, which already does this correctly.
    let (control_sender, control_receiver) = bounded(1);
    context
        .command_handle()
        .register(Box::from(ChannelBasedController::new(control_sender)));

    match control_receiver.recv_timeout(cfg.duration.to_std()?) {
        Ok(StreamControlMessage::Terminate) => return terminate(),
        Ok(StreamControlMessage::Pause) => loop {
            match control_receiver.recv() {
                Ok(StreamControlMessage::Terminate) => return terminate(),
                Ok(StreamControlMessage::Resume) => break,
                Ok(StreamControlMessage::Pause) => {}
                Err(_) => return terminate(),
            }
        },
        Ok(StreamControlMessage::Resume) | Err(_) => {}
    }
    context.output.send(Value::Empty)?;
    Ok(())
}

#[signature(
    control.bg,
    output = Known(ValueType::Empty),
    short = "Resume a paused job, letting it continue running in the background.",
    long = "Unlike a job started with a trailing `&` (which is in the background from the",
    long = "moment it starts), `bg` acts on a job that already exists and is currently",
    long = "paused (e.g. via `crush:pause`) -- it resumes it without putting it in the",
    long = "foreground the way `fg` would.",
)]
struct Bg {
    #[description("the job id of the paused job to resume in the background.")]
    job: usize,
}

fn bg(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Bg::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.global_state.resume(cfg.job.into())?;
    context.output.send(Value::Empty)
}

#[signature(
    control.fg,
    short = "Return the output of a background pipeline",
    long = "A job started with a trailing `&` runs in the background and registers its",
    long = "eventual result for later retrieval; `fg` waits for and returns that result.",
    example = "# Create a pipe",
    example = "$pipe := $($(table_input_stream value=$integer):pipe)",
    example = "# Create a job that writes 100_000 integers to the pipe and put this job in the background",
    example = "seq 100_000 | pipe:write &",
    example = "# Create a second job that reads from the pipe and sums all the integers and put this job in the background",
    example = "$sum_job_handle := $(pipe:read | sum &)",
    example = "# Close the pipe so that the second job can finish",
    example = "pipe:close",
    example = "# Put the sum job in the foreground",
    example = "fg $sum_job_handle",
)]
struct Fg {
    #[description("the job id of the background job to put into the foreground.")]
    job: Option<usize>,
}

fn fg(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Fg::parse(context.remove_arguments(), &context.global_state.printer())?;
    match cfg.job {
        None => match context.global_state.take_last_background_job() {
            None => context.output.send(Value::Empty),
            Some(v) => context.output.send(v.recv()?),
        },

        Some(id) => match context.global_state.take_background_job(id.into()) {
            None => context.output.send(Value::Empty),
            Some(v) => context.output.send(v.recv()?),
        },
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
    files: Vec<BinaryInput>,
}

fn source(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Source = Source::parse(context.remove_arguments(), &context.global_state.printer())?;
    for el in cfg.files {
        match el {
            BinaryInput::File(path) => {
                crate::execute::file(
                    &context.scope,
                    &path,
                    &context.output,
                    &context.global_state,
                )?;
            }
            BinaryInput::Glob(glob) => {
                let mut paths = Vec::new();
                glob.glob_files(&cwd()?, &mut paths)?;
                for path in paths {
                    crate::execute::file(
                        &context.scope,
                        &path,
                        &context.output,
                        &context.global_state,
                    )?;
                }
            }
            BinaryInput::String(string) => {
                crate::execute::string(
                    &context.scope,
                    &string,
                    LanguageMode::Command,
                    &context.output,
                    &context.global_state,
                    Background,
                )?;
            }
            BinaryInput::Regex(regex) => {
                let mut paths = Vec::new();
                regex.match_files(&cwd()?, &mut paths)?;
                for path in paths {
                    crate::execute::file(
                        &context.scope,
                        &path,
                        &context.output,
                        &context.global_state,
                    )?;
                }
            }
            BinaryInput::BinaryInputStream(mut stream) => {
                let mut string = String::new();
                stream.read_to_string(&mut string)?;
                crate::execute::string(
                    &context.scope,
                    &string,
                    LanguageMode::Command,
                    &context.output,
                    &context.global_state,
                    Background,
                )?;
            }
            BinaryInput::Binary(vec) => {
                let string = String::from_utf8_lossy(&vec);
                crate::execute::string(
                    &context.scope,
                    &string,
                    LanguageMode::Command,
                    &context.output,
                    &context.global_state,
                    Background,
                )?;
            }
        }
    }
    Ok(())
}

#[signature(
    control.which,
    short = "Find the path of an executable",
    long = "`which` searches the directories of the `$crush:end[PATH]` enivornment variable list for the specified command and returns the path to the first match.",
    output = Known(ValueType::File),
    example = "# Returns '/bin/ps'",
    example = "which ps",
)]
struct Which {
    #[description("the name of the command to find.")]
    command: String,
}

fn which(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Which::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::from(
        resolve_external_command(&cfg.command)?
            .ok_or_else(|| format!("Could not find the command `{}` on your path", &cfg.command))?,
    ))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "control",
        "Commands for flow control, (loops, etc)",
        Box::new(move |env| {
            r#if::If::declare(env)?;
            r#match::Match::declare(env)?;
            r#try::Try::declare(env)?;
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
            Assert::declare(env)?;
            Sleep::declare(env)?;
            Bg::declare(env)?;
            Fg::declare(env)?;
            help::HelpSignature::declare(env)?;
            Source::declare(env)?;
            Which::declare(env)?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
