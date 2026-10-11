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
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::{Pid, getpgrp, tcsetpgrp};
use signature::signature;
use std::borrow::BorrowMut;
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Stdio};

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

/// One transition `spawn_child_wait`'s waiter thread observed in the child, decoded
/// from a `WUNTRACED` `waitpid` call so a stop (e.g. the kernel suspending a
/// backgrounded job that tried to read/write the controlling terminal, or a real
/// Ctrl-Z reaching a foregrounded one directly) is reported instead of silently waited
/// through -- plain `Child::wait()` has no stable way to ask for that.
enum ChildEvent {
    /// The child was stopped (SIGSTOP, SIGTSTP, SIGTTIN, SIGTTOU, ...) but hasn't
    /// exited. May be followed by more `Stopped` events (each real stop/resume/stop
    /// cycle) before an eventual `Done`.
    Stopped,
    /// The child has exited or been killed; the waiter thread is finished.
    Done(CrushResult<()>),
}

/// Waits for `child` (identified by `pid`, already spawned onto its own tracked
/// `cmd:wait` thread that owns it and reports its state transitions via `events`)
/// while reacting to `control` the same way `schedule`'s `wait_for_command` does --
/// except here Pause and Terminate must reach the real OS process this job spawned,
/// not just this waiting thread: `cmd`'s child runs independently of any crush-level
/// job/thread tracking, so without sending it an actual signal, `pause`/
/// `terminate` on this job would stop waiting on the child without the child
/// itself ever stopping or exiting. SIGSTOP/SIGCONT suspend and resume the process;
/// SIGKILL is used (rather than SIGTERM) for Terminate so this matches the immediate,
/// unconditional semantics `terminate()` has everywhere else in the job control
/// system.
///
/// `holds_terminal` is true whenever this specific invocation currently has the real
/// controlling terminal handed over to the child's own process group (see
/// `cmd_internal` for the initial handoff, and the `ResumeForeground` arms below for a
/// later one via `fg`) -- only ever true for a command using the real tty, and only
/// when the handoff actually succeeded. It can change over the life of one call (a
/// `Stopped` event always gives the terminal back; a `ResumeForeground` may take it
/// again), so this returns its final value alongside the result rather than assuming
/// it's fixed for the whole call -- see the `wait_for_child` wrapper below, which uses
/// that to decide whether it still needs to hand the terminal back once this inner
/// loop is done.
fn wait_for_child_inner(
    pid: Pid,
    events: &Receiver<ChildEvent>,
    control: &Receiver<StreamControlMessage>,
    context: &CommandContext,
    holds_terminal: bool,
    crush_pgid: Pid,
) -> (CrushResult<()>, bool) {
    select! {
        recv(events) -> res => match res {
            Ok(ChildEvent::Done(r)) => (r, holds_terminal),
            Ok(ChildEvent::Stopped) => {
                // Give crush's own prompt the terminal back immediately, rather than
                // only once this whole call eventually returns -- a job that's stopped
                // (however that happened) but not yet resumed could sit like this for
                // a long time, during which crush itself needs to be able to read the
                // terminal again.
                if holds_terminal {
                    let _ = tcsetpgrp(&std::io::stdin(), crush_pgid);
                }
                // Reuses the exact same machinery pause/a real Ctrl-Z on *crush
                // itself* already use, so every other job-control-aware piece (Job::eval's
                // own recv_or_pause, jobs' status column, a later fg/bg) sees this
                // exactly like any other pause -- regardless of whether it was crush that
                // asked for it (see the Pause arm below) or the kernel that imposed it
                // (e.g. a backgrounded job touching the terminal, or Ctrl-Z reaching a
                // foregrounded job directly once it owns the terminal).
                let _ = context.global_state.pause(context.command_handle().job_handle.id());
                wait_for_child_inner(pid, events, control, context, false, crush_pgid)
            }
            Err(err) => (Err(err.into()), holds_terminal),
        },
        recv(control) -> message => match message {
            Ok(StreamControlMessage::Terminate) => {
                let _ = signal::kill(pid, signal::Signal::SIGKILL);
                (terminate(), holds_terminal)
            }
            Ok(StreamControlMessage::Resume) => {
                wait_for_child_inner(pid, events, control, context, holds_terminal, crush_pgid)
            }
            Ok(StreamControlMessage::ResumeForeground) => {
                // `fg` on a job that was never using the real tty at all (an ordinary
                // pipeline stage, say) just lands here as a harmless no-op: `pid` is
                // still the process's own pgid (see `process_group(0)` in
                // `cmd_internal`), so `tcsetpgrp` either succeeds pointlessly (nothing
                // was reading the terminal anyway) or fails and is ignored.
                let now_holds = tcsetpgrp(&std::io::stdin(), pid).is_ok();
                let _ = signal::kill(pid, signal::Signal::SIGCONT);
                wait_for_child_inner(pid, events, control, context, now_holds, crush_pgid)
            }
            Ok(StreamControlMessage::Pause) => {
                // If this job currently owns the terminal (e.g. pause on an
                // actively-running foreground command), give it back to crush's own
                // process group now -- the same reasoning as the Stopped arm above,
                // just triggered by crush itself asking for the pause rather than the
                // kernel imposing it.
                if holds_terminal {
                    let _ = tcsetpgrp(&std::io::stdin(), crush_pgid);
                }
                let _ = signal::kill(pid, signal::Signal::SIGSTOP);
                let mut now_holds = false;
                loop {
                    match control.recv() {
                        Ok(StreamControlMessage::Resume) => {
                            let _ = signal::kill(pid, signal::Signal::SIGCONT);
                            break;
                        }
                        Ok(StreamControlMessage::ResumeForeground) => {
                            now_holds = tcsetpgrp(&std::io::stdin(), pid).is_ok();
                            let _ = signal::kill(pid, signal::Signal::SIGCONT);
                            break;
                        }
                        Ok(StreamControlMessage::Terminate) => {
                            let _ = signal::kill(pid, signal::Signal::SIGKILL);
                            return (terminate(), now_holds);
                        }
                        Ok(StreamControlMessage::Pause) => {}
                        Err(_) => return (terminate(), now_holds),
                    }
                }
                // The SIGSTOP above caused exactly one real stop, which the waiter
                // thread will separately have reported as its own `ChildEvent::Stopped`
                // -- already accounted for here via this same Pause/Resume exchange, so
                // discard it rather than letting the next iteration mistake it for a
                // *new*, independent stop (e.g. a fresh Ctrl-Z reaching the child
                // directly) immediately after this resume. A `Done` racing in at the
                // same time is genuine and must not be lost, so only a `Stopped` is
                // discarded.
                if let Ok(ChildEvent::Done(r)) = events.try_recv() {
                    return (r, now_holds);
                }
                wait_for_child_inner(pid, events, control, context, now_holds, crush_pgid)
            }
            Err(err) => (Err(err.into()), holds_terminal),
        },
    }
}

fn wait_for_child(
    pid: Pid,
    events: &Receiver<ChildEvent>,
    control: &Receiver<StreamControlMessage>,
    context: &CommandContext,
    holds_terminal: bool,
) -> CrushResult<()> {
    let crush_pgid = getpgrp();
    let (result, still_holds_terminal) =
        wait_for_child_inner(pid, events, control, context, holds_terminal, crush_pgid);
    if still_holds_terminal {
        let _ = tcsetpgrp(&std::io::stdin(), crush_pgid);
    }
    result
}

/// Spawns a dedicated thread that owns `child` and does the actual blocking wait,
/// reporting every state transition (stopped, and finally exited/killed) through the
/// returned receiver -- so the caller can race that against the job's control channel
/// via `wait_for_child` instead of blocking on `child.wait()` directly (which, like
/// `schedule`'s old `cmd.eval(...)` call, would leave this job's control channel
/// unreachable for the entire lifetime of the child). Uses a raw `waitpid` with
/// `WUNTRACED` rather than `Child::wait()`: the latter has no stable way to ask for a
/// stop to be reported at all, so a stopped (not exited) child would otherwise just
/// leave this call blocked indefinitely, with nothing to report a stop happened.
fn spawn_child_wait(
    context: &CommandContext,
    child: Child,
) -> CrushResult<Receiver<ChildEvent>> {
    let (event_sender, event_receiver) = bounded(8);
    let pid = Pid::from_raw(child.id() as i32);
    // Cloned/copied out of `context` up front (rather than capturing `context` itself,
    // which isn't `'static`) so this thread can call `GlobalState::terminate` below --
    // see the `Signaled` arm's own comment for why.
    let global_state = context.global_state.clone();
    let job_id = context.command_handle().job_handle.id();
    context.global_state.threads().spawn(
        "cmd:wait",
        &context.next_command_handle(),
        move || {
            // Keeps `child` (and so its underlying pid) alive for the duration --
            // waited on directly by pid below rather than via `Child::wait()`, since
            // that has no stable way to request `WUNTRACED`.
            let _child = child;
            loop {
                let event = match waitpid(pid, Some(WaitPidFlag::WUNTRACED)) {
                    Ok(WaitStatus::Stopped(_, _)) => ChildEvent::Stopped,
                    Ok(WaitStatus::Exited(_, _)) => ChildEvent::Done(Ok(())),
                    Ok(WaitStatus::Signaled(_, _, _)) => {
                        // The child died because it received a real OS signal directly
                        // (e.g. via `host:signal ... SIGKILL`), entirely bypassing
                        // crush's own job-control plumbing -- unlike crush's own
                        // `terminate`, nothing has called `GlobalState::terminate` for
                        // this job, so nothing has marked it `Terminated` yet, and
                        // `jobs` would otherwise just silently stop listing it with no
                        // indication it was ever killed. Call it here instead, while
                        // this thread's own still-registered `CommandHandle` guarantees
                        // `job_control_data` is still alive (see `JobData::
                        // terminated_snapshot`'s doc comment) -- this is a no-op beyond
                        // that status bookkeeping: the process is already dead, so the
                        // stray `StreamControlMessage::Terminate` this also fans out to
                        // this job's other registered controllers (e.g. a redundant
                        // `signal::kill`, in `wait_for_child_inner` below) is harmless.
                        let _ = global_state.terminate(job_id);
                        ChildEvent::Done(Ok(()))
                    }
                    Ok(_) => continue,
                    Err(err) => ChildEvent::Done(Err(err.into())),
                };
                let done = matches!(event, ChildEvent::Done(_));
                let _ = event_sender.send(event);
                if done {
                    break;
                }
            }
            Ok(())
        },
    )?;
    Ok(event_receiver)
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
    // Every external command gets its own process group -- the same thing a real
    // shell's own job control does for every job it launches -- rather than silently
    // sharing crush's, which is what let a backgrounded job that touches the terminal
    // (e.g. `emacs &`) simply take it over instead of being stopped by the kernel like
    // it should be. `0` means "make the child its own group leader" (its pgid becomes
    // its own pid).
    cmd.process_group(0);
    // Crush itself ignores SIGTTOU/SIGTTIN for its own process (see
    // `interactive::ignore_terminal_control_signals`) -- otherwise its own terminal
    // operations while momentarily not the foreground group (e.g. reclaiming the
    // terminal after a foreground child exits) would fail outright with EIO rather than
    // just working. `SIG_IGN` survives `exec` (unlike a caught handler, which resets to
    // default), so without this the child would silently inherit that same "ignore"
    // disposition -- exactly undoing the point of giving it its own process group: a
    // backgrounded child that's ignoring SIGTTOU no longer gets stopped when it writes
    // to the terminal, it just writes to it, displacing crush's own prompt all over
    // again. Resetting both to their default disposition here, after fork but before
    // exec, is the standard fix real shells use for the exact same reason.
    unsafe {
        cmd.pre_exec(|| {
            let _ = nix::sys::signal::signal(
                nix::sys::signal::Signal::SIGTTOU,
                nix::sys::signal::SigHandler::SigDfl,
            );
            let _ = nix::sys::signal::signal(
                nix::sys::signal::Signal::SIGTTIN,
                nix::sys::signal::SigHandler::SigDfl,
            );
            Ok(())
        });
    }

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
        // Only a genuinely foreground invocation (no trailing `&`, not paused
        // mid-flight -- see `JobContext::foreground`) hands the terminal over: the
        // caller is actually going to block waiting for this, so it's safe for the
        // child to own the terminal until it stops or exits. A backgrounded one keeps
        // the terminal with crush's own process group instead, so if it tries to
        // read/write it, the kernel stops it automatically (SIGTTIN/SIGTTOU) exactly
        // like a real shell's background job would -- crush itself is never affected,
        // since it was never handed away. `tcsetpgrp` failing (e.g. crush's own stdin
        // isn't actually a terminal right now) just means there's nothing to hand off
        // or reclaim.
        let holds_terminal = context.foreground && tcsetpgrp(&std::io::stdin(), pid).is_ok();
        let events = spawn_child_wait(&context, child)?;
        wait_for_child(pid, &events, &control_receiver, &context, holds_terminal)?;
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
        let events = spawn_child_wait(&context, child)?;
        wait_for_child(pid, &events, &control_receiver, &context, false)?;

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
