use crate::lang::command::Command;
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::ColumnVec;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::job_control::StreamControlMessage;
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::pipe::{TableInputStream, pipe};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::global_state::GlobalState;
use crate::lang::state::handles::JobType::Background;
use crate::lang::state::scope::Scope;
use crate::{
    lang::pipe::{TableOutputStream, unlimited_streams},
    lang::{data::table::Row, value::Value, value::ValueType},
};
use crossbeam::channel::{Receiver, Sender, unbounded};
use crossbeam::select;
use signature::signature;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The fan-out loop behind `spawn_control_fanout`, factored out so it's directly
/// testable without needing a full `CommandContext`/`ThreadStore` (it's a blocking
/// loop, so it still needs to run on its own thread to test -- just not necessarily
/// one spawned through the full crush job-control machinery). Re-sends every message
/// read from `control` to each of `senders`, in order, until either `control` itself
/// disconnects, or `shutdown` fires -- see `TableOutputStream::control`'s doc comment
/// for why every worker thread needs its own receiver fed from here rather than a
/// plain clone of `control`, and `spawn_control_fanout`'s own doc comment for why
/// `shutdown` (not just `control` disconnecting) is essential: stream:group's own
/// `control` is the whole job's shared one, which won't disconnect until every thread
/// under that job -- this one included -- has already exited. Without an independent
/// way out, this loop would still be waiting on `control` forever, and the job could
/// never actually finish.
fn broadcast_control(
    control: Receiver<StreamControlMessage>,
    senders: Vec<Sender<StreamControlMessage>>,
    shutdown: Receiver<()>,
) {
    loop {
        select! {
            recv(control) -> msg => match msg {
                Ok(m) => {
                    for s in &senders {
                        let _ = s.send(m);
                    }
                }
                Err(_) => return,
            },
            recv(shutdown) -> _ => return,
        }
    }
}

/// Spawns one dedicated `group:control-fanout` thread (see `broadcast_control`'s own
/// doc comment for the loop itself) and returns `count` receivers, each independently
/// seeing every message `control` does -- unlike a plain `Receiver::clone()`, which
/// only ever delivers each message to whichever one clone claims it first (see
/// `TableOutputStream::control`'s doc comment). Used to give every one of
/// stream:group's worker threads its own working copy of the job's control channel
/// instead of the single shared one `CommandContext::initialize_output` registered --
/// without this, only one of the workers could ever actually be paused/terminated.
///
/// Also returns a `Sender<()>` the caller must fire once every one of the `count`
/// workers this fan-out feeds has finished -- see `broadcast_control`'s own doc comment
/// for why that's essential rather than optional cleanup.
fn spawn_control_fanout(
    context: &CommandContext,
    control: Receiver<StreamControlMessage>,
    count: usize,
) -> CrushResult<(Vec<Receiver<StreamControlMessage>>, Sender<()>)> {
    let mut senders = Vec::with_capacity(count);
    let mut receivers = Vec::with_capacity(count);
    for _ in 0..count {
        let (s, r) = unbounded();
        senders.push(s);
        receivers.push(r);
    }
    let (shutdown_sender, shutdown_receiver) = crossbeam::channel::bounded(1);
    context.global_state.threads().spawn(
        "group:control-fanout",
        &context.next_command_handle(),
        move || {
            broadcast_control(control, senders, shutdown_receiver);
            Ok(())
        },
    )?;
    Ok((receivers, shutdown_sender))
}

#[signature(
    stream.group,
    can_block = true,
    short = "Group stream by the specified column(s)",
    example = "# Group files in current tree by the number of hardlinks pointing to them, show",
    example = "# the number of files and the sum total file size for each link count. Sort results",
    example = "# by size.",
    example = "files --recurse | group links file_count=$count size={sum size} | sort size",
)]
pub struct Group {
    #[unnamed()]
    #[description("the column(s) to group by and copy into the output stream.")]
    group_by: Vec<String>,
    #[named()]
    #[description(
        "create additional columns by aggregating the grouped rows using the supplied aggregation command. The supplied command will be called once for each group, with a table_input_stream containing all rows within that group. Whatever the command outputs will be the value for the specified column for that group."
    )]
    command: OrderedStringMap<Command>,
}

fn aggregate(
    commands: Vec<Command>,
    context: &CommandContext,
    global_state: GlobalState,
    scope: Scope,
    destination: TableOutputStream,
    task_input: Receiver<(Vec<Value>, TableInputStream)>,
) -> CrushResult<()> {
    while let Ok((key, rows)) = task_input.recv() {
        match commands.len() {
            0 => {
                destination.send(Row::new(key))?;
            }
            1 => {
                let (input_sender, input_receiver) = pipe();
                let (output_sender, output_receiver) = pipe();
                input_sender.send(Value::TableInputStream(rows))?;
                drop(input_sender);
                commands[0].eval(
                    CommandContext::new(
                        &scope,
                        &global_state,
                        &context.source,
                        context.next_command_handle(),
                        Background,
                    )
                    .with_input(input_receiver)
                    .with_output(output_sender),
                )?;
                let mut result = key;
                result.push(output_receiver.recv()?);
                destination.send(Row::new(result))?;
            }
            _ => {
                let mut receivers = Vec::with_capacity(commands.len());
                let mut streams = Vec::with_capacity(commands.len());
                for command in &commands {
                    let (input_sender, input_receiver) = pipe();
                    let (output_sender, output_receiver) = pipe();
                    streams.push(input_sender.initialize(rows.types())?);

                    let local_command = command.clone();
                    let local_scope = scope.clone();
                    let local_state = global_state.clone();
                    let local_source = context.source.clone();
                    let warning_state = local_state.clone();
                    let next_id = context.next_command_handle();
                    let new_context = CommandContext::new(
                        &local_scope,
                        &local_state,
                        &local_source,
                        next_id,
                        Background,
                    )
                    .with_input(input_receiver)
                    .with_output(output_sender);

                    context.global_state.threads().spawn(
                        "group:aggr",
                        &context.next_command_handle(),
                        move || {
                            if let Err(e) = local_command.eval(new_context) {
                                warning_state.warn(&e);
                            }
                            Ok(())
                        },
                    )?;
                    receivers.push(output_receiver);
                }

                while let Ok(row) = rows.recv() {
                    for stream in streams.iter() {
                        let _ = stream.send(row.clone());
                    }
                }
                drop(streams);

                let mut result = key;
                for receiver in receivers {
                    result.push(receiver.recv()?);
                }
                destination.send(Row::new(result))?;
            }
        }
    }
    Ok(())
}

fn create_worker_thread(
    cfg: &Group,
    scope: &Scope,
    destination: &TableOutputStream,
    task_input: &Receiver<(Vec<Value>, TableInputStream)>,
    context: &CommandContext,
    global_state: &GlobalState,
    remaining_workers: Arc<AtomicUsize>,
    fanout_shutdown: Option<Sender<()>>,
) -> CrushResult<()> {
    let my_commands: Vec<Command> = cfg
        .command
        .iter()
        .map(|(_name, cmd)| cmd.clone())
        .collect::<Vec<_>>();
    let my_scope = scope.clone();
    let my_input = task_input.clone();
    let my_destination = destination.clone();
    let my_context = context.clone();
    let my_state = global_state.clone();
    context.global_state.threads().spawn(
        "group:collect",
        &context.next_command_handle(),
        move || {
            let warning_state = my_state.clone();
            if let Err(e) = aggregate(
                my_commands,
                &my_context,
                my_state,
                my_scope,
                my_destination,
                my_input,
            ) {
                warning_state.warn(&e);
            }
            // The last worker to finish tells the fan-out thread it can stop -- see
            // broadcast_control's own doc comment for why it can't just wait for
            // `control` (the job's shared one) to disconnect on its own.
            if remaining_workers.fetch_sub(1, Ordering::SeqCst) == 1 {
                if let Some(shutdown) = &fanout_shutdown {
                    let _ = shutdown.send(());
                }
            }
            Ok(())
        },
    )?;
    Ok(())
}

pub fn group(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Group::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut input = context.input_stream()?;
    let input_type = input.types().to_vec();
    let indices: Vec<usize> = cfg
        .group_by
        .iter()
        .map(|f| input_type.as_slice().find(f))
        .collect::<CrushResult<Vec<_>>>()?;

    if indices.is_empty() {
        return command_error("No group-by column specified");
    }

    let mut output_type = indices
        .iter()
        .map(|input_idx| input_type[*input_idx].clone())
        .collect::<Vec<_>>();

    for name in cfg.command.keys() {
        output_type.push(ColumnType::new_from_string(name.clone(), ValueType::Any));
    }

    let output_type = output_type.as_slice().deduplicate_names();
    const WORKER_COUNT: usize = 16;

    let output = context.initialize_output(&output_type)?;
    let mut groups: HashMap<Vec<Value>, TableOutputStream> = HashMap::new();

    let (task_output, task_input) = unbounded::<(Vec<Value>, TableInputStream)>();

    // Every worker needs its own dedicated copy of the job's control channel, not a
    // plain clone of `output`'s -- see spawn_control_fanout's and
    // TableOutputStream::control's doc comments for why a shared clone would only ever
    // let one of the sixteen workers actually respond to a pause/terminate.
    let (worker_controls, fanout_shutdown): (Vec<Option<Receiver<StreamControlMessage>>>, Option<Sender<()>>) =
        match output.control() {
            Some(control) => {
                let (receivers, shutdown) = spawn_control_fanout(&context, control, WORKER_COUNT)?;
                (receivers.into_iter().map(Some).collect(), Some(shutdown))
            }
            None => ((0..WORKER_COUNT).map(|_| None).collect(), None),
        };
    let remaining_workers = Arc::new(AtomicUsize::new(WORKER_COUNT));

    for worker_control in worker_controls {
        let worker_output = match worker_control {
            Some(control) => output.clone().with_control(control),
            None => output.clone(),
        };
        create_worker_thread(
            &cfg,
            &context.scope,
            &worker_output,
            &task_input,
            &context,
            &context.global_state,
            remaining_workers.clone(),
            fanout_shutdown.clone(),
        )?;
    }

    drop(task_input);

    while let Some(row) = input.next_row()? {
        let key = indices
            .iter()
            .map(|idx| row.cells()[*idx].clone())
            .collect::<Vec<_>>();
        let val = groups.get(&key);
        match val {
            None => {
                let (output_stream, input_stream) = unlimited_streams(input_type.to_vec());
                let _ = task_output.send((key.clone(), input_stream));
                let _ = output_stream.send(row);
                groups.insert(key, output_stream);
            }
            Some(output_stream) => {
                let _ = output_stream.send(row);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Regression test for the actual fix: every one of `count` receivers must see
    // every control message sent through `control`, not just whichever one happens to
    // claim it first (see TableOutputStream::control's doc comment, and
    // pipe::tests::plain_clone_of_an_interruptible_table_output_stream_does_not_broadcast_control
    // for why a plain Receiver clone -- what create_worker_thread used to hand every
    // stream:group worker -- can't do this). Ends the fan-out loop by dropping
    // control_sender after sending both messages, rather than racing a timeout to
    // detect "no more messages are coming".
    #[test]
    fn broadcast_control_delivers_every_message_to_every_receiver() {
        let (control_sender, control_receiver) = unbounded();
        let (_shutdown_sender, shutdown_receiver) = crossbeam::channel::bounded(1);
        let mut senders = Vec::new();
        let mut receivers = Vec::new();
        for _ in 0..5 {
            let (s, r) = unbounded();
            senders.push(s);
            receivers.push(r);
        }

        let handle =
            std::thread::spawn(move || broadcast_control(control_receiver, senders, shutdown_receiver));

        control_sender.send(StreamControlMessage::Pause).unwrap();
        control_sender.send(StreamControlMessage::Terminate).unwrap();
        drop(control_sender);

        handle.join().expect("broadcast_control thread panicked");

        for (i, r) in receivers.iter().enumerate() {
            let first = r
                .recv_timeout(Duration::from_secs(1))
                .unwrap_or_else(|_| panic!("receiver {i} never saw the Pause message"));
            assert!(
                matches!(first, StreamControlMessage::Pause),
                "receiver {i}'s first message should have been Pause"
            );
            let second = r
                .recv_timeout(Duration::from_secs(1))
                .unwrap_or_else(|_| panic!("receiver {i} never saw the Terminate message"));
            assert!(
                matches!(second, StreamControlMessage::Terminate),
                "receiver {i}'s second message should have been Terminate"
            );
        }
    }

    // Regression test for the deadlock this actually caused in practice (tests/group.crush
    // hung indefinitely once stream:group started using a real fan-out thread): `control`
    // is stream:group's own job-wide control channel, registered once for the whole job
    // and never disconnecting until every thread under that job -- the fan-out thread
    // included -- has already exited. Without `shutdown` as an independent way out,
    // broadcast_control would block on `control.recv()` forever in the overwhelmingly
    // common case where nothing ever pauses or terminates the job, and the job could
    // never finish. Bounded wait, not a direct join(): a real hang here must fail this
    // test with a clear message, not hang the whole test binary.
    #[test]
    fn broadcast_control_exits_once_shutdown_fires_even_if_control_never_disconnects() {
        let (_control_sender, control_receiver) = unbounded();
        // _control_sender is kept alive (never dropped) and never sent on, so
        // control_receiver can only ever be unblocked via shutdown.
        let (shutdown_sender, shutdown_receiver) = crossbeam::channel::bounded(1);

        let (done_tx, done_rx) = crossbeam::channel::bounded(1);
        std::thread::spawn(move || {
            broadcast_control(control_receiver, Vec::new(), shutdown_receiver);
            let _ = done_tx.send(());
        });

        shutdown_sender.send(()).unwrap();

        assert!(
            done_rx.recv_timeout(Duration::from_secs(5)).is_ok(),
            "broadcast_control did not exit within 5s of its shutdown channel firing, \
             even though control will never disconnect on its own -- this is the exact \
             deadlock a stream:group job could never finish from"
        );
    }
}
