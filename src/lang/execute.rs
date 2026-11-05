/// Functions that execute the contents of a string or file as Crush code.
use crate::lang::ast::lexer::LanguageMode;
use crate::lang::ast::source::SourceType::Input;
use crate::lang::ast::source::{Source, SourceType};
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::job::Job;
use crate::lang::pipe::{ValueSender, empty_channel, pipe};
use crate::lang::serialization::{deserialize, serialize};
use crate::lang::state::contexts::JobContext;
use crate::lang::state::global_state::GlobalState;
use crate::lang::state::handles::JobType;
use crate::lang::state::handles::JobType::Background;
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueDefinition};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::{fs, thread};

pub fn file(
    global_env: &Scope,
    filename: &Path,
    output: &ValueSender,
    global_state: &GlobalState,
) -> CrushResult<()> {
    let cmd = fs::read_to_string(filename)?;
    source(
        global_env,
        &Source::new(SourceType::File(filename.to_path_buf()), Arc::from(cmd)),
        LanguageMode::Command,
        output,
        global_state,
        Background,
    )
}

pub fn pup(env: Scope, buf: &Vec<u8>, global_state: &GlobalState) -> CrushResult<()> {
    let cmd = deserialize(buf, &env)?;
    match cmd {
        Value::Command(cmd) => {
            let (snd, recv) = pipe();

            let serializer_handle: JoinHandle<CrushResult<()>> = thread::Builder::new()
                .name("serializer".to_string())
                .spawn(move || {
                    let val = recv.recv()?;
                    let mut buf = Vec::new();
                    serialize(&val.materialize()?, &mut buf)?;
                    std::io::stdout().write(&buf)?;
                    Ok(())
                })?;

            let job = Job::new(
                vec![CommandInvocation::new(
                    ValueDefinition::Value(
                        Value::Command(cmd.clone()),
                        Source::new(Input, Arc::from("")),
                    ),
                    Source::new(Input, Arc::from("")),
                    vec![],
                )],
                Source::new(Input, Arc::from("")),
                false,
            );

            job.eval(JobContext::new(
                empty_channel(),
                snd.clone(),
                env.clone(),
                global_state.clone(),
                Background,
            ))?;
            global_state.threads().join(global_state.printer());
            let _ = serializer_handle.join();
            Ok(())
        }

        v => command_error(format!(
            "Expected a command, but found value of type `{}`",
            v.value_type()
        )),
    }
}

pub fn string(
    global_env: &Scope,
    command: &str,
    initial_mode: LanguageMode,
    output: &ValueSender,
    global_state: &GlobalState,
    job_type: JobType,
) -> CrushResult<()> {
    source(
        global_env,
        &Source::new(Input, Arc::from(command)),
        initial_mode,
        output,
        global_state,
        job_type,
    )
}

fn source(
    global_env: &Scope,
    command: &Source,
    initial_mode: LanguageMode,
    output: &ValueSender,
    global_state: &GlobalState,
    job_type: JobType,
) -> CrushResult<()> {
    let jobs = global_state
        .parser()
        .parse(command, &global_env, initial_mode)?;
    let last_idx = jobs.len().saturating_sub(1);
    for (idx, job_definition) in jobs.iter().enumerate() {
        // Only the final statement's value (or an earlier `return` -- see
        // `global_env.is_stopped()` below) is this call's own result; every other
        // statement's value is irrelevant and must not be forwarded to `output`. The
        // old code sent *every* statement's value onto the same shared, bounded(1)
        // `output` sender, which deadlocked once a third statement tried to send with
        // nothing ever having drained the first two (see
        // tests/source_three_statements.crush). `GlobalState::eval_statement` is the
        // same "last statement, or discard" dispatch `Closure::eval_inner` and
        // `JobListDefinition::eval` already use for the same reason; `output` is passed
        // straight through unchanged (same as before) so the final statement's own
        // `is_pipeline()`-ness is still accurate -- `eval_statement` only ever
        // substitutes anything for an earlier, discarded statement.
        let last = idx == last_idx;
        let job_context = JobContext::new(
            empty_channel(),
            output.clone(),
            global_env.clone(),
            global_state.clone(),
            job_type,
        );
        global_state.eval_statement(job_definition, job_context, last)?;

        if global_env.is_stopped() {
            break;
        }
    }
    Ok(())
}
