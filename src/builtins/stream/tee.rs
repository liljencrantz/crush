use crate::lang::command::Command;
use crate::lang::command::OutputType::Passthrough;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::pipe::{black_hole, pipe};
use crate::lang::state::contexts::CommandContext;
use signature::signature;

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

    for branch in branches {
        let (branch_sender, branch_receiver) = pipe();
        let branch_output = branch_sender.initialize(&types)?;
        let branch_context = context
            .empty()
            .with_input(branch_receiver)
            .with_output(black_hole());

        let thread_id = context.global_state.threads().spawn(
            "stream:tee",
            &context.next_command_handle(),
            move || branch.eval(branch_context),
        )?;

        branch_outputs.push(branch_output);
        branch_threads.push(thread_id);
    }

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

    // Dropping every branch's output stream closes its channel, which is how a
    // branch's own pipeline learns the input is exhausted.
    drop(branch_outputs);

    for id in branch_threads {
        if let Err(e) = context.global_state.threads().join_one(id) {
            context.global_state.warn(&e);
        }
    }

    Ok(())
}
