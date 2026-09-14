use crate::lang::ast::lexer::LanguageMode;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
use crate::lang::command_invocation::resolve_external_command;
use crate::lang::errors::{CrushResult, command_error, terminate, throw_error};
use crate::lang::job_control::{ChannelBasedController, StreamControlMessage};
use crate::lang::pipe::{empty_channel, pipe};
use crate::lang::signature::binary_input::BinaryInput;
use crate::lang::state::contexts::{CommandContext, JobContext};
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

#[signature(
    control.throw,
    can_block = false,
    short = "Raise a custom error, catchable and discriminable by its own error type.",
    long = "Unlike every other error in Crush, a thrown error's `type` (as seen via",
    long = "`catch {|$e| ...}`'s `$e:type`) is `error_type` itself, not a fixed name tied",
    long = "to whatever went wrong internally -- so a script or library can define and",
    long = "catch its own error categories.",
    output = Known(ValueType::Empty),
    example = "try { throw \"NotFound\" \"no such user\" } catch {|$e| assert ($e:type == \"NotFound\")}",
)]
struct Throw {
    #[description("the error's type, e.g. \"NotFound\". Visible to a catch block as `$e:type`.")]
    error_type: String,
    #[description("the error's message. Visible to a catch block as `$e:message`.")]
    message: String,
}

fn throw(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Throw = Throw::parse(context.remove_arguments(), &context.global_state.printer())?;
    throw_error(cfg.error_type, cfg.message)
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
    control.timeout,
    can_block = true,
    output = Unknown,
    short = "Run a command, terminating it if it hasn't finished within the given duration.",
    long = "If `command` finishes before `duration` elapses, `timeout` returns its result",
    long = "normally. Otherwise, a termination signal is sent to it -- the same cooperative",
    long = "mechanism `crush:terminate` uses, which only a command that actually checks for",
    long = "it (like `sleep`) will actually stop for; most builtins don't, and will keep",
    long = "running in the background regardless -- and `timeout` itself fails with a",
    long = "timeout error.",
    long = "",
    long = "A command left running this way isn't just background noise: crush waits for",
    long = "every spawned thread to finish before the whole session exits, so a `command`",
    long = "that never cooperates and never finishes on its own (an infinite loop, for",
    long = "example) will keep the script -- or the whole interactive session -- from",
    long = "exiting cleanly, even though `timeout` itself already returned its error.",
    example = "timeout $(duration:of seconds=5) {sleep $(duration:of seconds=30)}",
)]
struct Timeout {
    #[description("how long to let the command run before terminating it.")]
    duration: Duration,
    #[description("the command to run.")]
    command: Command,
}

fn timeout(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Timeout = Timeout::parse(context.remove_arguments(), &context.global_state.printer())?;

    let (child_output, child_input) = pipe();
    let job_id = context.command_handle().job_handle.id();

    // A genuinely separate, freshly nested job for the child -- not just this same job
    // reused (which `context.empty()` would give). This job's own can-block invocation
    // was itself spawned via ThreadStore::spawn, which registers a JobController under
    // this exact job/command id (see command.register(...) there); if `cfg.command`
    // were a bare, non-closure command, it would inherit that identical CommandHandle
    // and its own controller would land in the very same slot. This loop's own
    // termination calls below would then be unable to target the child without also
    // signaling this command's own outer registration -- confirmed as a real,
    // deterministically-reproducible bug (see tests/timeout_variable_duration.crush's
    // history): a stale self-directed Terminate sent while this loop is still running
    // sits queued until whoever later joins this command's own outer thread does so,
    // at which point InterruptibleJoinHandle::join()'s crossbeam::select! races it
    // against this function's real return value and can discard the real one. Giving
    // the child its own job side-steps the collision entirely, for both a closure body
    // (which already creates its own nested job or jobs regardless, via eval_inner) and
    // a bare command alike.
    let child_job = JobContext::new_nested(
        empty_channel(),
        child_output,
        context.scope.clone(),
        context.global_state.clone(),
        context.job_type,
        job_id,
    );
    let child_context = child_job.command_context(&context.source, vec![], None)?;
    // command_context() clones its own output sender into child_context rather than
    // moving it, so child_job would otherwise keep an extra, unused clone of
    // child_output alive for the rest of this function -- which would keep the
    // child_output/child_input pipe looking "connected" even after the real child
    // thread (and its own real clone) exits, defeating the disconnect detection below
    // entirely.
    drop(child_job);

    let thread_id = context.global_state.threads().spawn(
        "timeout",
        &context.next_command_handle(),
        move || cfg.command.eval(child_context),
    )?;

    match child_input.recv_timeout(cfg.duration.to_std()?) {
        Ok(v) => {
            context.global_state.threads().join_one(thread_id)?;
            context.output.send(v)
        }
        Err(e) if e.is_disconnected() => {
            // The command finished without ever sending a value (e.g. it errored) --
            // join it to surface that real error instead of this generic one.
            context.global_state.threads().join_one(thread_id)?;
            Ok(())
        }
        Err(_) => {
            // `command` runs as its own freshly nested job (`child_job` above), and
            // almost always creates further nested jobs of its own too (e.g. a
            // closure's `eval_inner` does this for every statement in its body) --
            // that's where a cooperating command like `sleep` actually registers its
            // controller. Reach every job descended from `job_id` (deliberately never
            // `job_id` itself -- see child_job's own comment above for why that's not
            // just unnecessary but actively wrong). Also resent periodically for a
            // grace window: a single attempt can race a command that hasn't finished
            // registering yet (e.g. one still spinning up on its spawned thread), which
            // would otherwise lose the signal and let a cooperating command run to
            // completion anyway. A command that never registers a controller at all
            // (most builtins, which aren't interruptible the way `sleep` is) never
            // receives any of these regardless, and just keeps running in the
            // background.
            let grace = std::time::Duration::from_secs(2);
            let poll = std::time::Duration::from_millis(20);
            let grace_start = std::time::Instant::now();
            loop {
                let jobs = context.global_state.jobs();
                for j in &jobs {
                    if j.id != job_id && is_descendant_job(&jobs, job_id, j.id) {
                        let _ = context.global_state.terminate(j.id);
                    }
                }
                match child_input.recv_timeout(poll) {
                    Ok(_) => {
                        let _ = context.global_state.threads().join_one(thread_id);
                        break;
                    }
                    Err(e) if e.is_disconnected() => {
                        let _ = context.global_state.threads().join_one(thread_id);
                        break;
                    }
                    Err(_) if grace_start.elapsed() < grace => {}
                    Err(_) => break,
                }
            }
            command_error("Command timed out")
        }
    }
}

/// True if `of` is a job running as part of `ancestor`, directly or transitively (e.g.
/// a closure/block body evaluated as part of an enclosing job) -- mirrors
/// `crush.rs`'s own `is_ancestor` helper, checked the other way around.
fn is_descendant_job(
    jobs: &[crate::lang::state::handles::JobInfo],
    ancestor: crate::lang::state::id::JobId,
    of: crate::lang::state::id::JobId,
) -> bool {
    let mut current = of;
    while let Some(parent) = jobs.iter().find(|j| j.id == current).and_then(|j| j.parent) {
        if parent == ancestor {
            return true;
        }
        current = parent;
    }
    false
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
        None,
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
            Throw::declare(env)?;
            Sleep::declare(env)?;
            Timeout::declare(env)?;
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
