use crate::lang::command::Command;
use crate::lang::errors::{CrushResult, to_crush_error, mandate, error};
use crate::lang::execution_context::CommandContext;
use signature::signature;
use std::process;
use crate::lang::value::Value;
use std::process::Stdio;
use std::io::{Write, Read};
use crate::lang::serialization::{serialize, deserialize};

#[signature(
sudo,
can_block = true,
short = "Execute a lambda as another user.",
example = "sudo {./foo:chown \"root\"} # Set owner of foo to root"
)]
pub struct Sudo {
    #[description("the command to run as another user.")]
    command: Command,
    #[description("the user to run the command as.")]
    #[default("root")]
    user: String,
}

/**
    Current implementation is crude and grossly inefficient.

    Firstly, it just shells out to the sudo command - which leads to potential visual problems with
    the terminal.

    Secondly, it creates 3 separate subthreads just to deal with stdin, stdout and stderr without
    blocking while the main thread waits for the command to exit. It is easy to do this much more
    efficiently, but this was the most straight forward implementation and the sudo command should
    never be run in a loop regardless.
 */
fn sudo(context: CommandContext) -> CrushResult<()> {
    let cfg: Sudo = Sudo::parse(context.arguments.clone(), &context.global_state.printer())?;
    let mut cmd = process::Command::new("sudo");
    let printer = context.global_state.printer().clone();

    cmd.arg("--user").arg(&cfg.user);
    cmd.arg("--").arg("crush").arg("--pup");
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = to_crush_error(cmd.spawn())?;
    let mut stdin = mandate(child.stdin.take(), "Expected stdin stream")?;
    let mut serialized = Vec::new();
    serialize(&Value::Command(cfg.command), &mut serialized)?;
    let threads = context.global_state.threads().clone();

    threads.spawn("sudo:stdin", move || {
        stdin.write(&serialized)?;
        Ok(())
    })?;

    let mut stdout = mandate(child.stdout.take(), "Expected output stream")?;
    let env = context.scope.clone();
    threads.spawn("sudo:stdout", move || {
        let _ = &context;
        let mut buff = Vec::new();
        to_crush_error(stdout.read_to_end(&mut buff))?;
        if buff.len() == 0 {
            error("No value returned")
        } else {
            context
                .output
                .send(deserialize(&buff, &env)?)
        }
    })?;

    let mut stderr = mandate(child.stderr.take(), "Expected error stream")?;
    threads.spawn("sudo:stderr", move || {
        let mut buff = Vec::new();
        to_crush_error(stderr.read_to_end(&mut buff))?;
        let errors = to_crush_error(String::from_utf8(buff))?;
        for e in errors.split('\n') {
            let err = e.trim();
            if !err.is_empty() {
                printer.error(err);
            }
        }
        Ok(())
    })?;

    child.wait()?;
    Ok(())
}
