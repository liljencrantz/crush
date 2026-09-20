use crate::CrushResult;
use crate::lang::argument::{Argument, SwitchStyle};
use crate::lang::command::OutputType::Known;
use crate::lang::command_invocation::resolve_external_command;
use crate::lang::errors::{command_error, terminate};
use crate::lang::job_control::{ChannelBasedController, StreamControlMessage};
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::value::Value;
use crate::lang::value::Value::{Binary, BinaryInputStream};
use crate::lang::value::ValueType;
use crate::state::contexts::CommandContext;
use crate::util::file::cwd;
use crossbeam::channel::{Receiver, bounded, select};
use nix::sys::signal;
use nix::unistd::Pid;
use signature::signature;
use std::borrow::BorrowMut;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ExitStatus, Stdio};

#[signature(
    control.cmd,
    short = "Execute an external command",
    long = "Globs are file-expanded. Argument and switch order is preserved.",
    output = Known(ValueType::BinaryInputStream),
    can_block = true,
)]
#[allow(unused)]
pub struct Cmd {
    #[description("The file path to the command to execute")]
    command: PathBuf,
    #[named()]
    #[description(
        "Switches to pass in to the command. The name will be prepended with a double dash '--', unless it is a single character name, in which case a single dash '-' will be prepended"
    )]
    switches: OrderedStringMap<Value>,
    #[unnamed()]
    #[description("Arguments to pass in to the command")]
    arguments: Vec<Value>,
}

fn format_value(v: &Value) -> CrushResult<Vec<String>> {
    Ok(v.clone()
        .materialize()?
        .to_string()
        .split("\n")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect())
}

/// Waits for `child` (identified by `pid`, already spawned onto its own tracked
/// `cmd:wait` thread that owns it and reports its exit via `done`) while reacting to
/// `control` the same way `schedule`'s `wait_for_command` does -- except here Pause and
/// Terminate must reach the real OS process this job spawned, not just this waiting
/// thread: `cmd`'s child runs independently of any crush-level job/thread tracking, so
/// without sending it an actual signal, `crush:pause`/`crush:terminate` on this job
/// would stop waiting on the child without the child itself ever stopping or exiting.
/// SIGSTOP/SIGCONT suspend and resume the process; SIGKILL is used (rather than
/// SIGTERM) for Terminate so this matches the immediate, unconditional semantics
/// `terminate()` has everywhere else in the job control system.
fn wait_for_child(
    pid: Pid,
    done: &Receiver<CrushResult<ExitStatus>>,
    control: &Receiver<StreamControlMessage>,
) -> CrushResult<()> {
    select! {
        recv(done) -> res => match res {
            Ok(Ok(_status)) => Ok(()),
            Ok(Err(err)) => Err(err),
            Err(err) => Err(err.into()),
        },
        recv(control) -> message => match message {
            Ok(StreamControlMessage::Terminate) => {
                let _ = signal::kill(pid, signal::Signal::SIGKILL);
                terminate()
            }
            Ok(StreamControlMessage::Resume) => wait_for_child(pid, done, control),
            Ok(StreamControlMessage::Pause) => {
                let _ = signal::kill(pid, signal::Signal::SIGSTOP);
                loop {
                    match control.recv() {
                        Ok(StreamControlMessage::Resume) => {
                            let _ = signal::kill(pid, signal::Signal::SIGCONT);
                            break;
                        }
                        Ok(StreamControlMessage::Terminate) => {
                            let _ = signal::kill(pid, signal::Signal::SIGKILL);
                            return terminate();
                        }
                        Ok(StreamControlMessage::Pause) => {}
                        Err(_) => return terminate(),
                    }
                }
                wait_for_child(pid, done, control)
            }
            Err(err) => Err(err.into()),
        },
    }
}

/// Spawns a dedicated thread that owns `child` and does the actual blocking `wait()`,
/// reporting its result through the returned receiver -- so the caller can race that
/// against the job's control channel via `wait_for_child` instead of blocking on
/// `child.wait()` directly (which, like `schedule`'s old `cmd.eval(...)` call, would
/// leave this job's control channel unreachable for the entire lifetime of the child).
fn spawn_child_wait(
    context: &CommandContext,
    mut child: Child,
) -> CrushResult<Receiver<CrushResult<ExitStatus>>> {
    let (done_sender, done_receiver) = bounded(1);
    context.global_state.threads().spawn(
        "cmd:wait",
        &context.next_command_handle(),
        move || {
            let res = child.wait().map_err(|e| e.into());
            let _ = done_sender.send(res);
            Ok(())
        },
    )?;
    Ok(done_receiver)
}

fn cmd_internal(
    context: CommandContext,
    file: PathBuf,
    mut arguments: Vec<Argument>,
) -> CrushResult<()> {
    let (control_sender, control_receiver) = bounded(1);
    let control = Box::from(ChannelBasedController::new(control_sender));
    context.command_handle().register(control);

    let use_tty = !context.input.is_pipeline() && !context.output.is_pipeline();
    let mut cmd = std::process::Command::new(file.as_os_str());

    for a in arguments.drain(..) {
        match a.argument_type {
            None => match a.value {
                Value::Glob(glob) => {
                    let mut files = Vec::new();
                    glob.glob_files(&cwd()?, &mut files)?;
                    for file in files {
                        cmd.arg(file);
                    }
                }
                _ => {
                    for s in format_value(&a.value)? {
                        cmd.arg(s);
                    }
                }
            },

            Some(name) => {
                let (switch, join_string) = match a.switch_style {
                    SwitchStyle::None => {
                        if name.len() == 1 {
                            (format!("-{}", name), "")
                        } else {
                            (format!("--{}", name), "=")
                        }
                    }
                    SwitchStyle::Single => (format!("-{}", name), ""),
                    SwitchStyle::Double => (format!("--{}", name), "="),
                };
                match a.value {
                    Value::Bool(true) => {
                        cmd.arg(switch);
                    }
                    Value::Glob(glob) => {
                        let mut files = Vec::new();
                        glob.glob_files(&cwd()?, &mut files)?;
                        for file in files {
                            cmd.arg(format!(
                                "{}{}{}",
                                switch,
                                join_string,
                                file.to_str().ok_or("Invalid file name")?
                            ));
                        }
                    }
                    _ => {
                        for s in format_value(&a.value)? {
                            cmd.arg(format!("{}{}{}", switch, join_string, s));
                        }
                    }
                }
            }
        }
    }

    if use_tty {
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let child = cmd.spawn()?;
        let pid = Pid::from_raw(child.id() as i32);
        let done = spawn_child_wait(&context, child)?;
        wait_for_child(pid, &done, &control_receiver)?;
        context.output.send(Value::Empty)
    } else {
        let input = context.input.recv()?;

        let (stdout_reader, stdout_writer) = os_pipe::pipe()?;
        let (mut stderr_reader, stderr_writer) = os_pipe::pipe()?;

        cmd.stdin(Stdio::piped());
        cmd.stdout(stdout_writer);
        cmd.stderr(stderr_writer);

        let mut child = cmd.spawn()?;
        let mut stdin = child.stdin.take().ok_or("Expected stdin stream")?;

        match input {
            Value::Empty => {
                drop(stdin);
            }
            Binary(v) => {
                context.global_state.threads().spawn(
                    "cmd:stdin",
                    &context.next_command_handle(),
                    move || {
                        stdin.write(&v)?;
                        Ok(())
                    },
                )?;
            }
            BinaryInputStream(mut r) => {
                context.global_state.threads().spawn(
                    "cmd:stdin",
                    &context.next_command_handle(),
                    move || {
                        std::io::copy(r.as_mut(), stdin.borrow_mut())?;
                        Ok(())
                    },
                )?;
            }
            _ => return command_error("Invalid input: Expected binary data"),
        }

        context
            .output
            .send(BinaryInputStream(Box::from(stdout_reader)))?;
        let my_context = context.clone();
        context.global_state.threads().spawn(
            "cmd:stderr",
            &context.next_command_handle(),
            move || {
                let _ = &my_context;
                let mut buff = Vec::new();
                stderr_reader.read_to_end(&mut buff)?;
                let errors = String::from_utf8(buff)?;
                for e in errors.split('\n') {
                    let err = e.trim();
                    if !err.is_empty() {
                        my_context.global_state.printer().error(err);
                    }
                }
                Ok(())
            },
        )?;

        let pid = Pid::from_raw(child.id() as i32);
        let done = spawn_child_wait(&context, child)?;
        wait_for_child(pid, &done, &control_receiver)?;

        Ok(())
    }
}

fn cmd(mut context: CommandContext) -> CrushResult<()> {
    let mut arguments = context.remove_arguments();
    if arguments.is_empty() {
        return command_error("No command given");
    }
    match arguments.remove(0).value {
        Value::File(f) => {
            let file = if f.exists() {
                Some(f.to_path_buf())
            } else {
                resolve_external_command(f.to_str().ok_or("Invalid command name")?)?
            };

            if let Some(file) = file {
                cmd_internal(context, file, arguments)
            } else {
                command_error(format!(
                    "Unknown command {}",
                    f.to_str().unwrap_or("<encoding error>")
                ))
            }
        }
        Value::String(s) => {
            if let Some(file) = resolve_external_command(s.as_ref())? {
                cmd_internal(context, file, arguments)
            } else {
                command_error(format!("Unknown command `{}`", s))
            }
        }

        _ => command_error("Not a valid command"),
    }
}
