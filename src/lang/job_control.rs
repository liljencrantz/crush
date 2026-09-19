use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crossbeam::channel::{Receiver, Sender, unbounded};
use crossbeam::select;
use itertools::Either;
use std::thread::ThreadId;

pub trait JobControl {
    fn terminate(&self) -> CrushResult<()>;
    fn pause(&self) -> CrushResult<()>;
    fn resume(&self) -> CrushResult<()>;
}

pub type JobController = Box<dyn JobControl + Send>;

pub struct ChannelBasedController(Sender<StreamControlMessage>);

impl ChannelBasedController {
    pub fn new(sender: Sender<StreamControlMessage>) -> Self {
        ChannelBasedController(sender)
    }
}

impl JobControl for ChannelBasedController {
    fn terminate(&self) -> CrushResult<()> {
        Ok(self.0.send(StreamControlMessage::Terminate)?)
    }

    fn pause(&self) -> CrushResult<()> {
        Ok(self.0.send(StreamControlMessage::Pause)?)
    }

    fn resume(&self) -> CrushResult<()> {
        Ok(self.0.send(StreamControlMessage::Resume)?)
    }
}

#[derive(Clone, Copy)]
pub enum StreamControlMessage {
    Terminate,
    Pause,
    Resume,
}

pub struct InterruptibleJoinHandle<T> {
    result_receiver: Receiver<T>,
    control_receiver: Receiver<StreamControlMessage>,
    id: ThreadId,
    name: Option<String>,
}

impl<T> InterruptibleJoinHandle<T> {
    pub fn new(
        id: ThreadId,
        name: Option<&str>,
        result_receiver: Receiver<T>,
        control_receiver: Receiver<StreamControlMessage>,
    ) -> Self {
        InterruptibleJoinHandle {
            id,
            name: name.map(|x| x.to_string()),
            result_receiver,
            control_receiver,
        }
    }

    pub fn id(&self) -> ThreadId {
        self.id
    }

    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    pub fn join(&self) -> CrushResult<Either<T, StreamControlMessage>> {
        select! {
            recv(self.result_receiver) -> result => match result {
                Ok(res) => Ok(Either::Left(res)),
                Err(err) => Err(err.into()),
            },
            recv(self.control_receiver) -> control => match control {
                Ok(msg) => Ok(Either::Right(msg)),
                Err(err) => Err(err.into()),
            },
        }
    }

    /// Like `join`, but never blocks: `None` means the thread hasn't finished (or sent a
    /// control message) yet, in which case nothing was consumed and a later `join`/
    /// `try_join` call can still observe it.
    pub fn try_join(&self) -> Option<CrushResult<Either<T, StreamControlMessage>>> {
        if let Ok(res) = self.result_receiver.try_recv() {
            return Some(Ok(Either::Left(res)));
        }
        if let Ok(msg) = self.control_receiver.try_recv() {
            return Some(Ok(Either::Right(msg)));
        }
        None
    }
}

/// The fan-out loop behind `spawn_control_fanout`, factored out so it's directly
/// testable without needing a full `CommandContext`/`ThreadStore` (it's a blocking
/// loop, so it still needs to run on its own thread to test -- just not necessarily
/// one spawned through the full crush job-control machinery). Re-sends every message
/// read from `control` to each of `senders`, in order, until either `control` itself
/// disconnects, or `shutdown` fires -- see `TableOutputStream::control`'s doc comment
/// for why every worker/branch thread needs its own receiver fed from here rather than
/// a plain clone of `control`, and `spawn_control_fanout`'s own doc comment for why
/// `shutdown` (not just `control` disconnecting) is essential: the caller's `control` is
/// the whole job's shared one, which won't disconnect until every thread under that job
/// -- this one included -- has already exited. Without an independent way out, this
/// loop would still be waiting on `control` forever, and the job could never actually
/// finish.
pub fn broadcast_control(
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

/// Spawns one dedicated fan-out thread, named `name` (e.g. `"group:control-fanout"`,
/// `"tee:control-fanout"`), and returns `count` receivers, each independently seeing
/// every message `control` does -- unlike a plain `Receiver::clone()`, which only ever
/// delivers each message to whichever one clone claims it first (see
/// `TableOutputStream::control`'s doc comment). Used to give every one of a command's
/// several worker/branch threads its own working copy of the job's control channel
/// instead of the single shared one `CommandContext::initialize_output` registered --
/// without this, only one of them could ever actually be paused/terminated (this is
/// exactly the bug stream:group had, and stream:tee's branches had, before each grew
/// its own call to this).
///
/// Also returns a `Sender<()>` the caller must fire once every one of the `count`
/// workers/branches this fan-out feeds has finished -- see `broadcast_control`'s own doc
/// comment for why that's essential rather than optional cleanup.
pub fn spawn_control_fanout(
    context: &CommandContext,
    name: &str,
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
        name,
        &context.next_command_handle(),
        move || {
            broadcast_control(control, senders, shutdown_receiver);
            Ok(())
        },
    )?;
    Ok((receivers, shutdown_sender))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Regression test for the actual fix: every one of `count` receivers must see
    // every control message sent through `control`, not just whichever one happens to
    // claim it first (see TableOutputStream::control's doc comment, and
    // pipe::tests::plain_clone_of_an_interruptible_table_output_stream_does_not_broadcast_control
    // for why a plain Receiver clone -- what stream:group's create_worker_thread used to
    // hand every worker, before this existed -- can't do this). Ends the fan-out loop
    // by dropping control_sender after sending both messages, rather than racing a
    // timeout to detect "no more messages are coming".
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
    // is the job's own job-wide control channel, registered once for the whole job and
    // never disconnecting until every thread under that job -- the fan-out thread
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
