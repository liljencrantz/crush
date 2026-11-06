use crate::lang::command::Command;
use crate::lang::errors::{CrushResult, terminate};
use crate::lang::job_control::{ChannelBasedController, StreamControlMessage};
use crate::lang::pipe::pipe;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use chrono::{Duration, Local};
use crossbeam::channel::{Receiver, bounded, select};
use signature::signature;
use std::sync::Arc;

#[signature(
    control.timeit,
    short = "Execute a command many times and estimate the execution time.",
    example = "timeit {files|sort size}",
)]
pub struct TimeIt {
    #[description("the command to time.")]
    it: Command,

    #[description(
        "the number of runs in each repeat. If unspecified, timeit will repeat enough times for each batch to take roughly 0.4 seconds."
    )]
    number: Option<usize>,

    #[description("repeat count. The average speed in the fastest repeat will be returned.")]
    #[default(5usize)]
    repeat: usize,
}

/// Blocks until `done` produces the timed run's own result, while still reacting to
/// `control` the same way `schedule`'s own `wait_for_command` does: Pause blocks this
/// thread (not the timed run's own thread, which keeps running) until Resume or
/// Terminate, and Terminate aborts immediately without waiting for the run to actually
/// finish. This is what makes `timeit` responsive to job control even while a single
/// timed invocation is still in flight, not just in the gaps between repeats.
fn wait_for_run(
    done: &Receiver<CrushResult<()>>,
    control: &Receiver<StreamControlMessage>,
) -> CrushResult<()> {
    select! {
        recv(done) -> res => match res {
            Ok(inner) => inner,
            Err(err) => Err(err.into()),
        },
        recv(control) -> message => match message {
            Ok(StreamControlMessage::Terminate) => terminate(),
            Ok(StreamControlMessage::Resume) | Ok(StreamControlMessage::ResumeForeground) => {
                wait_for_run(done, control)
            }
            Ok(StreamControlMessage::Pause) => {
                loop {
                    match control.recv() {
                        Ok(StreamControlMessage::Terminate) => return terminate(),
                        Ok(StreamControlMessage::Resume)
                        | Ok(StreamControlMessage::ResumeForeground) => break,
                        Ok(StreamControlMessage::Pause) => {}
                        Err(_) => return terminate(),
                    }
                }
                wait_for_run(done, control)
            }
            Err(err) => Err(err.into()),
        },
    }
}

pub fn time_run(
    it: &Command,
    context: &CommandContext,
    control: &Receiver<StreamControlMessage>,
) -> CrushResult<Duration> {
    let (sender, reciever) = pipe();

    let handle = context.command_handle().clone();
    let c = context.global_state.threads().spawn(
        "output consumer",
        &context.next_command_handle(),
        move || {
            let res = reciever.recv()?;
            if let Ok(mut stream) = res.stream(&handle) {
                while let Some(_) = stream.next_row()? {}
            }
            Ok(())
        },
    )?;

    let start_time = Local::now();
    // Each repeat must be a genuinely fresh invocation, not a reuse of `context`'s own
    // `input` -- `context.input` (typically `empty_channel()`) is deliberately single-shot
    // (one pre-loaded value, sender dropped right after), which is correct for an ordinary
    // one-shot job but breaks the moment the same context is reused for a second run: an
    // external command (`cmd_internal`'s non-tty branch) reads its input exactly once via
    // `context.input.recv()?`, so the second repeat found that channel already drained and
    // disconnected. `context.empty()` gives every repeat its own fresh input (and clears
    // arguments/this the same way `with_args(vec![], None)` did), only overriding `output`.
    //
    // Run on its own thread, raced against `control` via `wait_for_run`, rather than
    // called directly on this thread: `it` is arbitrary user-supplied code that can
    // block for as long as it likes, and without this, nothing here would ever notice
    // a `pause`/`terminate` sent to this job until that one run happened to finish on
    // its own.
    let local_it = it.clone();
    let local_context = context.empty().with_output(sender);
    let (done_sender, done_receiver) = bounded(1);
    let run_handle = context.global_state.threads().spawn(
        "timeit:run",
        &context.next_command_handle(),
        move || {
            let res = local_it.eval(local_context);
            let _ = done_sender.send(res);
            Ok(())
        },
    )?;
    wait_for_run(&done_receiver, control)?;
    context
        .global_state
        .printer()
        .handle_error(context.global_state.threads().join_one(run_handle.id()));
    context
        .global_state
        .printer()
        .handle_error(context.global_state.threads().join_one(c.id()));
    let end_time = Local::now();
    Ok(end_time - start_time)
}

fn repeatn(
    it: &Command,
    context: &CommandContext,
    n: usize,
    control: &Receiver<StreamControlMessage>,
) -> CrushResult<Duration> {
    let mut times = Vec::new();
    for _ in 0..n {
        times.push(time_run(it, context, control)?);
    }
    let sum: Duration = times.iter().fold(Duration::seconds(0), |a, b| a + *b);
    Ok(sum / (times.len() as i32))
}

fn timeit(mut context: CommandContext) -> CrushResult<()> {
    let cfg: TimeIt = TimeIt::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.output.clone();
    let mut repeat_times = Vec::new();

    let (control_sender, control_receiver) = bounded(1);
    let control = Arc::from(ChannelBasedController::new(control_sender));
    context.command_handle().register(control);

    let number = match cfg.number {
        Some(n) => n,
        None => {
            let mut n = 1usize;
            while repeatn(&cfg.it, &context, n, &control_receiver)? * (n as i32)
                < Duration::milliseconds(400)
            {
                n *= 2;
            }
            n
        }
    };
    for _ in 0..cfg.repeat {
        repeat_times.push(repeatn(&cfg.it, &context, number, &control_receiver)?);
    }
    let tm = repeat_times
        .into_iter()
        .min()
        .ok_or("Failed to run command")?;

    output.send(Value::Duration(tm))
}
