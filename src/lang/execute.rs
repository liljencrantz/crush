use crate::lang::errors::{argument_error_legacy, to_crush_error, CrushResult};
use crate::lang::execution_context::{CommandContext, JobContext};
use crate::lang::data::scope::Scope;
use crate::lang::serialization::{deserialize, serialize};
use crate::lang::pipe::{pipe, empty_channel, ValueSender};
use crate::lang::value::Value;
use std::io::Write;
use std::path::Path;
use std::{fs};
use crate::lang::global_state::GlobalState;

pub fn file(
    global_env: &Scope,
    filename: &Path,
    output: &ValueSender,
    global_state: &GlobalState,
) -> CrushResult<()> {
    let cmd = to_crush_error(fs::read_to_string(filename))?;
    string(global_env, &cmd.as_str(), output, global_state)
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
                move || {
                    let val = recv.recv()?;
                    let mut buf = Vec::new();
                    serialize(&val.materialize()?, &mut buf)?;
                    to_crush_error(std::io::stdout().write(&buf))?;
                    Ok(())
                },
            )?;

            cmd.invoke(CommandContext::new(&env, global_state));
            global_state.threads().join(global_state.printer());

            Ok(())
        }

        _ => argument_error_legacy("Expected a command, but found other value"),
    }
}

pub fn string(
    global_env: &Scope,
    command: &str,
    output: &ValueSender,
    global_state: &GlobalState,
) -> CrushResult<()> {
    let jobs = global_state.parser().parse(command, &global_env)?;
    for job_definition in jobs {
        let handle = job_definition.invoke(JobContext::new(
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
