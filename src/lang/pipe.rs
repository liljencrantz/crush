/**
This file implements the crush equivalent of a pipe from a regular shell.

Unlike normal pipes, these pipes can send *any* crush value, but they are limited to sending data
between threads inside of a single process. The most important use case is to send a single value
of the type TableInputStream.
 */
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::Row;
use crate::lang::errors::{CrushError, CrushResult, error, terminate};
use crate::lang::job_control::{ChannelBasedController, JobController, StreamControlMessage};
use crate::lang::pipe::SenderType::{BlackHole, LastElement, Pipeline, Printer};
use crate::lang::state::handles::JobHandle;
use crate::lang::threads::ThreadStore;
use crate::lang::value::Value;
use chrono::Duration;
use crossbeam::channel::{Receiver, Select, Sender, bounded, unbounded};
use crossbeam::select;

#[derive(Clone)]
enum SenderType {
    LastElement(Sender<Value>),
    Printer(Sender<Value>),
    Pipeline(Sender<Value>),
    BlackHole,
}

#[derive(Clone)]
pub struct ValueSender {
    sender_type: SenderType,
}

impl ValueSender {
    pub fn send(&self, cell: Value) -> CrushResult<()> {
        match &self.sender_type {
            LastElement(s) | Printer(s) | Pipeline(s) => Ok(s.send(cell)?),
            BlackHole => Ok(()),
        }
    }

    pub fn empty(&self) -> CrushResult<()> {
        self.send(Value::Empty)
    }

    pub fn initialize(&self, signature: &[ColumnType]) -> CrushResult<TableOutputStream> {
        let (output, input) = streams(signature.to_vec())?;
        self.send(Value::TableInputStream(input))?;
        Ok(output)
    }

    pub fn is_pipeline(&self) -> bool {
        match self.sender_type {
            LastElement(_) => false,
            Printer(_) => false,
            Pipeline(_) => true,
            BlackHole => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValueReceiver {
    receiver: Receiver<Value>,
    is_pipeline: bool,
}

impl ValueReceiver {
    pub fn recv(&self) -> CrushResult<Value> {
        Ok(self.receiver.recv()?)
    }

    pub fn recv_timeout(&self, timeout: std::time::Duration) -> CrushResult<Value> {
        Ok(self.receiver.recv_timeout(timeout)?)
    }

    pub fn is_pipeline(&self) -> bool {
        self.is_pipeline
    }

    /// The standard way for a command whose subject can be given either as an argument
    /// (`cmd $value`) or piped in (`$value | cmd`) to read that subject: if a pipe is
    /// actually connected, the value flowing through it always wins, regardless of
    /// whether `value` (typically the command's own optional argument) was also given
    /// -- a command later in a pipeline can't opt out of receiving its predecessor's
    /// output, so preferring the pipe is the only choice that can't silently ignore
    /// real input. With no pipe connected, `value` is used, and it's an error for
    /// neither to be present. Keeping this in one place (rather than every such command
    /// reimplementing the same `if is_pipeline() {...} else {...}` check) is what keeps
    /// them all actually consistent -- see `dir`/`member`/`typeof` for callers.
    pub fn recv_or(&self, value: Option<Value>) -> CrushResult<Value> {
        if self.is_pipeline() {
            self.recv()
        } else {
            value.ok_or_else(|| "No value specified".into())
        }
    }
}

/**
A Sender that will drop any data sent to it at once.
 */
pub fn black_hole() -> ValueSender {
    ValueSender {
        sender_type: BlackHole,
    }
}

/**
A receiver that when read will return a single instance of Value::Empty
 */
pub fn empty_channel() -> ValueReceiver {
    let (o, mut i) = pipe();
    let _ = o.send(Value::Empty);
    i.is_pipeline = false;
    i
}

#[derive(Clone)]
pub struct TableOutputStream {
    sender: Sender<Row>,
    control: Option<Receiver<StreamControlMessage>>,
    types: Vec<ColumnType>,
}

impl TableOutputStream {
    pub fn send(&self, row: Row) -> CrushResult<()> {
        match &self.control {
            None => Ok(self.sender.send(row)?),
            Some(control) => {
                select! {
                    send(self.sender, row) -> res => Ok(res?),
                    recv(control) -> message => match message {
                        Ok(StreamControlMessage::Terminate) => {
                            terminate()
                        }
                        Ok(StreamControlMessage::Resume) => {
                            self.send(row)
                        }

                        Ok(StreamControlMessage::Pause) => {
                           loop {
                                match control.recv()? {
                                    StreamControlMessage::Terminate => {
                                        return terminate();
                                    }
                                    StreamControlMessage::Pause => {}
                                    StreamControlMessage::Resume => break,
                                }
                            }
                            self.send(row)
                        }
                        Err(err) => {Err(err.into())}
                    },
                }
            }
        }
    }

    /// Handle any pending control message, e.g. from the user pressing Ctrl-C, without sending a
    /// row. `send` does this as part of sending, but a command that waits for a long time between
    /// rows, like one waiting for events, needs to check in between to stop in time. Returns an
    /// error if the job has been terminated, and blocks while it is paused.
    pub fn poll_control(&self) -> CrushResult<()> {
        let Some(control) = &self.control else {
            return Ok(());
        };
        loop {
            match control.try_recv() {
                Ok(StreamControlMessage::Terminate) => return terminate(),
                Ok(StreamControlMessage::Resume) => {}
                Ok(StreamControlMessage::Pause) => loop {
                    match control.recv()? {
                        StreamControlMessage::Terminate => return terminate(),
                        StreamControlMessage::Pause => {}
                        StreamControlMessage::Resume => break,
                    }
                },
                Err(_) => return Ok(()),
            }
        }
    }

    pub fn types(&self) -> &[ColumnType] {
        &self.types
    }

    pub fn interruptible(self) -> (TableOutputStream, JobController) {
        let (control_sender, control_receiver) = unbounded();
        (
            TableOutputStream {
                sender: self.sender,
                control: Some(control_receiver),
                types: self.types,
            },
            Box::from(ChannelBasedController::new(control_sender)),
        )
    }

    /// This stream's own control channel, if any -- a clone of the `Receiver` half, not
    /// a new subscription. A plain `TableOutputStream::clone()` (as when fanning a
    /// single interruptible stream out across several worker threads, the way
    /// `stream:group` does) clones this along with everything else, and a crossbeam
    /// `Receiver` clone is another handle onto the *same* queue, not a broadcast
    /// subscription -- a control message sent once is only ever delivered to whichever
    /// one clone's `select!`/`try_recv()` claims it first, never to every clone. A
    /// caller that needs every one of several concurrent consumers to see every message
    /// needs its own fan-out (see `stream::group`'s `spawn_control_fanout`), built from
    /// this getter plus `with_control`.
    pub fn control(&self) -> Option<Receiver<StreamControlMessage>> {
        self.control.clone()
    }

    /// Replaces this stream's control channel -- the write-side counterpart of
    /// `control()`, for handing a clone its own dedicated receiver (e.g. one leg of a
    /// fan-out) instead of the shared one an ordinary `clone()` would have given it.
    pub fn with_control(mut self, control: Receiver<StreamControlMessage>) -> Self {
        self.control = Some(control);
        self
    }
}

#[derive(Clone)]
pub struct TableInputStream {
    receiver: Receiver<Row>,
    types: Vec<ColumnType>,
    /// The job (if any) still responsible for producing this stream's rows, tagged by
    /// `GlobalState::recv_job_result` when it hands back a `Value::TableInputStream`
    /// without waiting for that job to fully finish first. Checked once the stream is
    /// actually drained to the end -- see `resolve_end_of_stream_error`.
    ///
    /// A *cloned `JobHandle`*, not just its bare `JobId`: `JobId` numbers are recycled
    /// (`GlobalState`'s `next_id` hands out the smallest currently-unused one) the moment
    /// nothing still holds a strong reference to that job's `JobHandle` -- which, once
    /// this same producer job's threads finish and get joined (removing their
    /// `ThreadData`, the only other thing keeping it alive), would otherwise be *this
    /// tag itself*. If the id got recycled and reassigned to some unrelated later job
    /// before this stream is ever drained, joining "by id" would wait on the wrong job
    /// entirely -- confirmed in practice as a real, reproducible deadlock where a job's
    /// own recycled id got reassigned to the very job trying to read its result. Holding
    /// the handle keeps the id reserved for as long as this tag can still reference it.
    ///
    /// Shared (`Arc<Mutex<...>>`), not a plain field: `TableInputStream` is `Clone`, and
    /// a stream captured into a variable (e.g. `$a := $(...)`) commonly gets cloned again
    /// for actual reading (`Value::stream()`'s `interruptible()` wrapper clones it) --
    /// every one of those clones must see the *same* tag, and in particular must all see
    /// it cleared once resolved. A plain, per-clone `Option` would let the reading
    /// clone's own copy resolve (and get reaped) while `$a`'s own separate copy keeps
    /// holding its own `JobHandle` clone forever, keeping the job's `JobControlData`
    /// alive and reporting it as still live (e.g. to `crush:exit`) even though its
    /// stream had already been fully, successfully drained.
    producer: Arc<Mutex<Option<(JobHandle, ThreadStore)>>>,
}

impl TableInputStream {
    /// Tags this stream with the job still producing it, so a later real read-to-the-end
    /// can join that job's threads and surface a trailing error instead of silently
    /// treating early termination as clean end-of-stream -- see
    /// `GlobalState::recv_job_result`'s doc comment for the full explanation of why this
    /// is deferred rather than done eagerly, and this struct's own `producer` field doc
    /// for why a full `JobHandle` is held rather than just its `JobId`.
    pub fn with_producer_job(self, job: JobHandle, threads: ThreadStore) -> Self {
        *self.producer.lock().unwrap() = Some((job, threads));
        self
    }

    /// A disconnected channel looks identical whether the producer finished cleanly or
    /// failed partway through, after already having handed back this stream's handle
    /// (see `GlobalState::recv_job_result`'s doc comment for why that handoff can happen
    /// before the producer is actually done). If this stream was tagged with the job
    /// still producing it, and `err` really is a disconnection (not e.g. `recv_timeout`
    /// simply running out of time with the producer still legitimately running -- joining
    /// here would incorrectly block on that instead of just reporting the timeout), join
    /// every thread under that job now -- by construction a genuine disconnection only
    /// happens once they've all actually exited, so this never blocks on anything that
    /// isn't already finished -- and surface a real error from any of them instead of
    /// `err`, the generic disconnection one `err` would otherwise be.
    ///
    /// The tag is taken (cleared), not just read, so this only ever joins once even if
    /// several clones of this same stream all reach end-of-stream (harmless but
    /// pointless to repeat), and so every clone sees it gone afterward -- see the
    /// `producer` field's own doc comment for why that sharing is essential.
    fn resolve_end_of_stream_error(&self, err: CrushError) -> CrushError {
        if !err.is_disconnected() {
            return err;
        }
        if let Some((job, threads)) = self.producer.lock().unwrap().take() {
            if let Err(real_err) = threads.join_job(job.id()) {
                return real_err;
            }
        }
        err
    }

    pub fn get(&self, idx: i128) -> CrushResult<Row> {
        let mut i = 0i128;
        loop {
            match self.recv() {
                Ok(row) => {
                    if i == idx {
                        return Ok(row);
                    }
                    i += 1;
                }
                Err(_) => return error("Index out of bounds"),
            }
        }
    }

    pub fn interruptible(&self) -> (Stream, JobController) {
        let (control_sender, control_receiver) = unbounded();

        (
            Box::from(InterruptibleTableInputStream {
                input: self.clone(),
                control: control_receiver,
            }),
            Box::from(ChannelBasedController::new(control_sender)),
        )
    }

    pub fn recv(&self) -> CrushResult<Row> {
        match self.receiver.recv() {
            Ok(row) => self.validate(row),
            Err(err) => Err(self.resolve_end_of_stream_error(err.into())),
        }
    }

    pub fn recv_timeout(&self, timeout: Duration) -> CrushResult<Row> {
        match self.receiver.recv_timeout(timeout.to_std().unwrap()) {
            Ok(row) => self.validate(row),
            Err(err) => Err(self.resolve_end_of_stream_error(err.into())),
        }
    }

    pub fn types(&self) -> &[ColumnType] {
        &self.types
    }

    fn validate(&self, row: Row) -> CrushResult<Row> {
        if row.cells().len() != self.types.len() {
            return error(format!(
                "Pipeline expected rows to have {} columns, but received row with {} columns.",
                self.types.len(),
                row.cells().len()
            ));
        }
        for (c, ct) in row.cells().iter().zip(self.types.iter()) {
            if !ct.cell_type.is(c) {
                return error(
                    format!(
                        "Pipeline expected column `{}` to be of type `{}`, but was of type `{}`.",
                        ct.name(),
                        ct.cell_type,
                        c.value_type(),
                    )
                    .as_str(),
                );
            }
        }
        Ok(row)
    }
}

/**
A Sender/Receiver pair that is bounded to only one Value on the wire before blocking.
 */
pub fn pipe() -> (ValueSender, ValueReceiver) {
    let (send, recv) = bounded(1);
    (
        ValueSender {
            sender_type: Pipeline(send),
        },
        ValueReceiver {
            receiver: recv,
            is_pipeline: true,
        },
    )
}

pub fn last_element() -> (ValueSender, ValueReceiver) {
    let (send, recv) = bounded(1);
    (
        ValueSender {
            sender_type: LastElement(send),
        },
        ValueReceiver {
            receiver: recv,
            is_pipeline: false,
        },
    )
}

struct InterruptibleTableInputStream {
    input: TableInputStream,
    control: Receiver<StreamControlMessage>,
}

impl TableStreamReader for InterruptibleTableInputStream {
    fn read(&mut self) -> CrushResult<Row> {
        loop {
            select! {
                // Delegating to self.input's own validate()/resolve_end_of_stream_error()
                // (rather than a raw `Ok(r?)`, which used to be here) matters for the
                // same reason TableInputStream::recv() does it: a disconnection here must
                // still be checked against this stream's tagged producer job, to recover
                // a real trailing error instead of reporting clean EOF, and to actually
                // join (and so reap) that job now that it's genuinely done -- otherwise a
                // stream consumed this way (e.g. by `count`, via `Value::stream()`) never
                // triggers either, leaving the producer's job registered as live forever.
                recv(self.input.receiver) -> r => return match r {
                    Ok(row) => self.input.validate(row),
                    Err(err) => Err(self.input.resolve_end_of_stream_error(err.into())),
                },
                recv(self.control) -> msg => {
                    match msg {
                        Ok(StreamControlMessage::Terminate) => { return terminate();}
                        Ok(StreamControlMessage::Pause) => {
                            loop {
                                match self.control.recv() {
                                Ok(StreamControlMessage::Terminate) => {
                                        return terminate();
                                        }
                                Ok(StreamControlMessage::Resume) => break,
                                Ok(StreamControlMessage::Pause) => {}
                                Err(_) => return terminate(),
                                }
                            }
                        }
                        Ok(StreamControlMessage::Resume) => {}
                        Err(e) => {
                            return Err(e.into());
                        }
                    }
                }
            }
        }
    }

    fn read_timeout(&mut self, timeout: Duration) -> CrushResult<Row> {
        let mut sel = Select::new();
        let oper1 = sel.recv(&self.input.receiver);
        let oper2 = sel.recv(&self.control);

        let oper = sel.select_timeout(timeout.to_std()?);
        match oper {
            Err(e) => Err(e.into()),
            Ok(oper) => match oper.index() {
                // See read()'s own comment on why this goes through validate()/
                // resolve_end_of_stream_error() rather than a raw `Ok(...?)`.
                i if i == oper1 => match oper.recv(&self.input.receiver) {
                    Ok(row) => self.input.validate(row),
                    Err(err) => Err(self.input.resolve_end_of_stream_error(err.into())),
                },
                i if i == oper2 => terminate(),
                _ => unreachable!(),
            },
        }
    }

    fn types(&self) -> &[ColumnType] {
        self.input.types()
    }
}

/**
A Sender/Receiver pair that is bounded to only one Value on the wire before blocking.
 */
pub fn printer_pipe() -> (ValueSender, ValueReceiver) {
    let (send, recv) = bounded(1);
    (
        ValueSender {
            sender_type: Printer(send),
        },
        ValueReceiver {
            receiver: recv,
            is_pipeline: false,
        },
    )
}

pub fn streams(signature: Vec<ColumnType>) -> CrushResult<(TableOutputStream, TableInputStream)> {
    let (output, input) = bounded(128);
    let mut seen = HashMap::new();

    for (idx, sig) in signature.iter().enumerate() {
        match seen.get(sig.name()) {
            Some(first_idx) => {
                return error(format!(
                    "Duplicate column name, column {} and column {} are both named `{}`", first_idx, idx, sig.name()));
            }
            None => {
                seen.insert(sig.name(), idx);
            },
        }
    }
    Ok((
        TableOutputStream {
            sender: output,
            types: signature.clone(),
            control: None,
        },
        TableInputStream {
            receiver: input,
            types: signature,
            producer: Arc::new(Mutex::new(None)),
        },
    ))
}

pub fn unlimited_streams(signature: Vec<ColumnType>) -> (TableOutputStream, TableInputStream) {
    let (output, input) = unbounded();
    (
        TableOutputStream {
            sender: output,
            types: signature.clone(),
            control: None,
        },
        TableInputStream {
            receiver: input,
            types: signature,
            producer: Arc::new(Mutex::new(None)),
        },
    )
}

/// A trait to allow reading from a TableInputStrem, a Table, a Dict, etc as a sequence of Row values.
pub trait TableStreamReader {
    fn read(&mut self) -> CrushResult<Row>;
    fn read_timeout(&mut self, timeout: Duration) -> CrushResult<Row>;
    fn types(&self) -> &[ColumnType];

    /// Read the next row, treating ordinary stream exhaustion as `Ok(None)` rather than
    /// an error.
    ///
    /// A disconnected/exhausted stream (`CrushError::is_disconnected()`) is the only
    /// outcome `read()` can produce that isn't either a row or a genuine error -- it's
    /// the normal way a stream signals "no more rows," whether that's because a
    /// materialized source (a `Table`, `Dict`, etc.) ran out of elements, or because a
    /// channel-backed stream's sender was dropped once its producer finished. Anything
    /// else `read()` returns -- an explicit `Terminate` interrupt, or a genuine data/
    /// validation error from `TableInputStream::recv()`'s schema check -- is a real
    /// condition the caller should see, not silently swallow. This is the replacement
    /// for the `while let Ok(row) = ... .read() { }` pattern used throughout the
    /// codebase, which conflates all three cases; use `while let Some(row) =
    /// ... .next_row()? { }` instead.
    fn next_row(&mut self) -> CrushResult<Option<Row>> {
        match self.read() {
            Ok(row) => Ok(Some(row)),
            Err(e) if e.is_disconnected() => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl TableStreamReader for TableInputStream {
    fn read(&mut self) -> Result<Row, CrushError> {
        self.recv()
    }

    fn read_timeout(&mut self, timeout: Duration) -> CrushResult<Row> {
        self.recv_timeout(timeout)
    }

    fn types(&self) -> &[ColumnType] {
        self.types()
    }
}

pub type Stream = Box<dyn TableStreamReader + Send>;

#[cfg(test)]
mod tests {
    use super::*;

    // Documents a permanent, deliberate limitation this crate's own code has to route
    // around -- see `control`/`with_control`'s doc comments and stream::group's
    // spawn_control_fanout for the actual fix built on top of them. A plain
    // TableOutputStream::clone() is derived, so `control`'s Receiver gets cloned along
    // with everything else -- but a crossbeam Receiver clone is another handle onto the
    // *same* queue, not a broadcast subscription: a message sent once through the one
    // registered JobController is only ever delivered to whichever one clone's
    // `try_recv()`/`select!` claims it, never to every clone. This is why
    // stream:group's worker pool (16 threads, all needing to react to the same job's
    // pause/terminate, all originally sharing a plain clone of one interruptible
    // TableOutputStream) could only ever pause/terminate one of its sixteen workers.
    //
    // Deliberately synchronous and deterministic, no threads or timing: calling
    // poll_control() on `output` first is what "whichever clone claims it first" means
    // here, so this doesn't need a race to demonstrate the behavior.
    #[test]
    fn plain_clone_of_an_interruptible_table_output_stream_does_not_broadcast_control() {
        let (output, controller) = unlimited_streams(vec![]).0.interruptible();
        let output2 = output.clone();

        controller.terminate().unwrap();

        let first_saw_terminate = output.poll_control().is_err();
        let second_saw_terminate = output2.poll_control().is_err();

        assert!(
            first_saw_terminate && !second_saw_terminate,
            "expected only the first clone polled to observe the one Terminate sent \
             (first={}, second={}) -- if this ever changes, TableOutputStream::clone() \
             itself must have started broadcasting control messages, which would make \
             stream::group's own fan-out workaround redundant (harmless, but worth \
             simplifying away) rather than necessary",
            first_saw_terminate,
            second_saw_terminate,
        );
    }
}
