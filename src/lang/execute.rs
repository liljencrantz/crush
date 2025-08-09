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
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueDefinition};
use std::{fs, thread};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::thread::JoinHandle;
use crate::lang::state::handles::JobType;
use crate::lang::state::handles::JobType::Background;

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
            );

            job.eval(JobContext::new(
                empty_channel(),
                snd.clone(),
                env.clone(),
                global_state.clone(),
                Background,
            ))?;
            global_state.threads().join(global_state.printer());
            serializer_handle.join();
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
        &Source::new(SourceType::Input, Arc::from(command)),
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
    for job_definition in jobs {
        let handle = job_definition.eval(JobContext::new(
            empty_channel(),
            output.clone(),
            global_env.clone(),
            global_state.clone(),
            job_type,
        ))?;
        handle.map(|id| global_state.threads().join_one(id, &global_state.printer()));
    }
    Ok(())
}
