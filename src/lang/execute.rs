/// Functions that execute the contents of a string or file as Crush code.

use crate::lang::errors::{argument_error_legacy, CrushResult};
use crate::lang::state::contexts::{CommandContext, JobContext};
use crate::lang::state::scope::Scope;
use crate::lang::serialization::{deserialize, serialize};
use crate::lang::pipe::{pipe, empty_channel, ValueSender};
use crate::lang::value::Value;
use std::io::Write;
use std::path::Path;
use std::{fs};
use crate::lang::ast::lexer::LexerMode;
use crate::lang::state::global_state::GlobalState;

pub fn file(
    global_env: &Scope,
    filename: &Path,
    output: &ValueSender,
    global_state: &GlobalState,
) -> CrushResult<()> {
    let cmd = fs::read_to_string(filename)?;
    string(global_env, &cmd.as_str(), LexerMode::Command, output, global_state)
}

pub fn pup(
    env: Scope,
    buf: &Vec<u8>,
    global_state: &GlobalState,
) -> CrushResult<()> {
    let cmd = deserialize(buf, &env)?;
    match cmd {
        Value::Command(cmd) => {
            let (snd, recv) = pipe();

            global_state.threads().spawn(
                "serializer",
                None,
                move || {
                    let val = recv.recv()?;
                    let mut buf = Vec::new();
                    serialize(&val.materialize()?, &mut buf)?;
                    std::io::stdout().write(&buf)?;
                    Ok(())
                },
            )?;

            cmd.eval(CommandContext::new(&env, global_state).with_output(snd))?;
            global_state.threads().join(global_state.printer());

            Ok(())
        }

        _ => argument_error_legacy("Expected a command, but found other value"),
    }
}

pub fn string(
    global_env: &Scope,
    command: &str,
    initial_mode: LexerMode,
    output: &ValueSender,
    global_state: &GlobalState,
) -> CrushResult<()> {
    let jobs = global_state.parser().parse(command, &global_env, initial_mode)?;
    for job_definition in jobs {
        let handle = job_definition.eval(JobContext::new(
            empty_channel(),
            output.clone(),
            global_env.clone(),
            global_state.clone(),
        ))?;

        handle.map(|id| global_state.threads()
            .join_one(
                id,
                &global_state.printer().with_source(command, job_definition.location()),
            ));
    }
    Ok(())
}
