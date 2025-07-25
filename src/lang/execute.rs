/// Functions that execute the contents of a string or file as Crush code.
use crate::lang::ast::lexer::LanguageMode;
use crate::lang::ast::source::{Source, SourceType};
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::pipe::{ValueSender, empty_channel, pipe};
use crate::lang::serialization::{deserialize, serialize};
use crate::lang::state::contexts::{CommandContext, JobContext};
use crate::lang::state::global_state::GlobalState;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

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
    )
}

pub fn pup(env: Scope, buf: &Vec<u8>, global_state: &GlobalState) -> CrushResult<()> {
    let cmd = deserialize(buf, &env)?;
    match cmd {
        Value::Command(cmd) => {
            let (snd, recv) = pipe();

            global_state.threads().spawn("serializer", None, move || {
                let val = recv.recv()?;
                let mut buf = Vec::new();
                serialize(&val.materialize()?, &mut buf)?;
                std::io::stdout().write(&buf)?;
                Ok(())
            })?;

            cmd.eval(
                CommandContext::new(
                    &env,
                    global_state,
                    &Source::new(SourceType::Input, Arc::from("")),
                )
                .with_output(snd),
            )?;
            global_state.threads().join(global_state.printer());

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
) -> CrushResult<()> {
    source(
        global_env,
        &Source::new(SourceType::Input, Arc::from(command)),
        initial_mode,
        output,
        global_state,
    )
}

fn source(
    global_env: &Scope,
    command: &Source,
    initial_mode: LanguageMode,
    output: &ValueSender,
    global_state: &GlobalState,
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
        ))?;

        handle.map(|id| global_state.threads().join_one(id, &global_state.printer()));
    }
    Ok(())
}
