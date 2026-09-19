use crate::lang::command::Command;
use crate::lang::command::OutputType::Passthrough;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::job_control::{StreamControlMessage, spawn_control_fanout};
use crate::lang::pipe::{black_hole, pipe};
use crate::lang::state::contexts::CommandContext;
use crossbeam::channel::{Receiver, Sender};
use signature::signature;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[signature(
    stream.tee,
    can_block = true,
    output = Passthrough,
    short = "Duplicate the input stream into one or more side pipelines, passing the original stream through unchanged.",
    long = "Each `branches` block receives its own independent copy of every row and runs",
    long = "to completion as its own pipeline, in parallel with the others; its own output",
    long = "is discarded, so a branch is only useful for its side effects (writing to a",
    long = "file, counting, etc). `tee`'s own output is the untouched original stream, so",
    long = "it composes directly into a larger pipeline. If a branch fails, the failure is",
    long = "reported via `crush:warn:list` rather than aborting `tee` or the other branches.",
    example = "# Write a snapshot to disk while continuing to filter the live stream",
    example = "host:procs | tee {json:to snapshot.json} | where {($cpu > 50)}",
)]
pub struct Tee {
    #[description("one or more pipelines to send an independent copy of the stream through.")]
    #[unnamed()]
    branches: Vec<Command>,
}

fn tee(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Tee = Tee::parse(context.remove_arguments(), &context.global_state.printer())?;
    if cfg.branches.is_empty() {
        return command_error("tee needs at least one branch.");
    }
    let branches = cfg.branches;

    let mut input = context.input_stream()?;
    let types = input.types().to_vec();
    let output = context.initialize_output(&types)?;

    // Every branch gets its own single-writer/single-reader stream, fed row by row
    // from the same upstream read loop below. Branches run on their own threads so a
    // slow branch can't stall the others or the main passthrough output.
    let mut branch_outputs = Vec::new();
    let mut branch_threads = Vec::new();

    // Every branch needs its own dedicated copy of the job's control channel, not a
    // plain clone of `output`'s -- see spawn_control_fanout's and
    // TableOutputStream::control's doc comments for why a shared clone would only ever
    // let one branch actually respond to a pause/terminate. Without this, a branch that
    // doesn't drain its input as fast as rows arrive (or doesn't drain it at all) fills
    // its stream's bounded buffer and blocks the loop below on a plain, uncontrolled
    // send -- which stalls this whole thread before it ever gets a chance to notice
    // anything sent to the job, including on tee's own already-interruptible
    // passthrough `output` below it in the same loop iteration.
    let (branch_controls, fanout_shutdown): (
        Vec<Option<Receiver<StreamControlMessage>>>,
        Option<Sender<()>>,
    ) = match output.control() {
        Some(control) => {
            let (receivers, shutdown) =
                spawn_control_fanout(&context, "tee:control-fanout", control, branches.len())?;
            (receivers.into_iter().map(Some).collect(), Some(shutdown))
        }
        None => ((0..branches.len()).map(|_| None).collect(), None),
    };
    let remaining_branches = Arc::new(AtomicUsize::new(branches.len()));

    for (branch, branch_control) in branches.into_iter().zip(branch_controls) {
        let (branch_sender, branch_receiver) = pipe();
        let branch_output = match branch_control {
            Some(control) => branch_sender.initialize(&types)?.with_control(control),
            None => branch_sender.initialize(&types)?,
        };
        let branch_context = context
            .empty()
            .with_input(branch_receiver)
            .with_output(black_hole());

        let remaining = remaining_branches.clone();
        let shutdown = fanout_shutdown.clone();
        let thread_id = context.global_state.threads().spawn(
            "stream:tee",
            &context.next_command_handle(),
            move || {
                let res = branch.eval(branch_context);
                // The last branch to finish tells the fan-out thread it can stop --
                // see broadcast_control's own doc comment for why it can't just wait
                // for `control` (the job's shared one) to disconnect on its own.
                if remaining.fetch_sub(1, Ordering::SeqCst) == 1 {
                    if let Some(shutdown) = &shutdown {
                        let _ = shutdown.send(());
                    }
                }
                res
            },
        )?;

        branch_outputs.push(branch_output);
        branch_threads.push(thread_id);
    }

    // Run the feeding loop in a closure so every exit path -- including an early
    // `return` below (e.g. a branch's send observing crush:terminate, see
    // TableOutputStream::send) or `input.next_row()`'s own `?` -- still reaches the
    // cleanup after it: every branch thread must be joined regardless of how/why this
    // command is done producing output, or a still-running (or, for something like an
    // infinite loop branch, permanently stuck) branch thread never gets joined at all
    // and leaks in ThreadStore forever.
    let result: CrushResult<()> = (|| {
        while let Some(row) = input.next_row()? {
            // A branch can finish (successfully or not) well before this loop is done
            // feeding it rows -- e.g. a branch whose command doesn't even exist fails on
            // its very first step, before ever reading any input, and its receiver is
            // dropped the moment its thread exits. Sending it another row then hits a
            // disconnected channel: entirely expected, exactly like a live `head`/`take`
            // truncating its own upstream elsewhere in the codebase, and not this branch's
            // real failure -- that's still recovered below, once its thread is actually
            // joined. Stop feeding *that* branch and keep going with the others; only a
            // genuine (non-disconnection) send error is this loop's own problem to report.
            let mut i = 0;
            while i < branch_outputs.len() {
                match branch_outputs[i].send(row.clone()) {
                    Ok(()) => i += 1,
                    Err(e) if e.is_send_disconnected() => {
                        branch_outputs.remove(i);
                    }
                    Err(e) => return Err(e),
                }
            }

            // Same idea for tee's own passthrough output: if whatever comes next in the
            // pipeline has already stopped reading (e.g. a downstream `head`), that's
            // benign too -- stop producing output nobody wants instead of failing.
            match output.send(row) {
                Ok(()) => {}
                Err(e) if e.is_send_disconnected() => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    })();

    // Dropping every branch's output stream closes its channel, which is how a
    // branch's own pipeline learns the input is exhausted.
    drop(branch_outputs);

    for id in branch_threads {
        if let Err(e) = context.global_state.threads().join_one(id) {
            context.global_state.warn(&e);
        }
    }

    result
}
