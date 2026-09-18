use crate::lang::errors::{CrushError, CrushResult, terminate};
use crate::lang::job_control::StreamControlMessage;
use crate::lang::state::handles::JobHandle;
use crate::lang::threads::ThreadStore;
use crossbeam::channel::{Receiver, Sender, bounded};
use crossbeam::select;
use std::cmp::min;
use std::collections::VecDeque;
use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io::{Error, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct ChannelReader {
    receiver: Receiver<Box<[u8]>>,
    buff: Option<Box<[u8]>>,
    /// The job (if any) still responsible for producing these bytes, tagged by
    /// `GlobalState::recv_job_result` when it hands back a `Value::BinaryInputStream`
    /// without waiting for that job to fully finish first. Checked once the channel
    /// disconnects -- see `read`'s own comment, and `TableInputStream`'s `producer`
    /// field doc comment (in `crate::lang::pipe`) for why this holds a full, cloned
    /// `JobHandle` rather than just its `JobId` -- a bare id can be recycled and
    /// reassigned to an unrelated later job the moment nothing else references it.
    ///
    /// Shared (`Arc<Mutex<...>>`), not a plain field, for the same reason
    /// `TableInputStream::producer` (in `crate::lang::pipe`) is: `clone()` below can
    /// produce another `ChannelReader` reading the same underlying bytes, and every
    /// clone must see the tag cleared once any one of them resolves it -- otherwise a
    /// clone that's never read again (e.g. a script variable still holding the original
    /// `Value::BinaryInputStream`) keeps its own copy of the `JobHandle` alive forever,
    /// even after another clone has fully, successfully drained the same bytes.
    producer: Arc<Mutex<Option<(JobHandle, ThreadStore)>>>,
    /// The job-control channel registered via `register_control`, if any -- see that
    /// method's own doc comment. `None` (the default) means a blocked `read()` behaves
    /// exactly as before: no control message can ever interrupt it.
    control: Option<Receiver<StreamControlMessage>>,
}

impl Debug for ChannelReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("<channel reader>") //.map_err(|e| std::fmt::Error::default())
    }
}

/// The two ways `ChannelReader::recv_next` can fail to produce a chunk of bytes.
enum RecvFailure {
    /// The data channel itself disconnected -- ordinary end of stream, or the producer
    /// failed; `read`'s own caller resolves which via the `producer` tag.
    Disconnected,
    /// A real, unrecoverable error: a `Terminate` control message, or (mirroring
    /// `TableOutputStream::send`'s identical choice) the control channel itself
    /// unexpectedly disconnecting. Smuggled through the `io::Error` this ultimately
    /// becomes -- see `read`'s own comment on why, and `CrushError`'s
    /// `From<std::io::Error>` for the matching unwrap.
    Interrupted(CrushError),
}

impl BinaryReader for ChannelReader {
    fn clone(&self) -> Box<dyn BinaryReader + Send + Sync> {
        Box::from(ChannelReader {
            receiver: self.receiver.clone(),
            buff: None,
            producer: self.producer.clone(),
            control: self.control.clone(),
        })
    }

    fn set_producer_job(&mut self, job: JobHandle, threads: ThreadStore) {
        *self.producer.lock().unwrap() = Some((job, threads));
    }

    fn register_control(&mut self, control: Receiver<StreamControlMessage>) {
        self.control = Some(control);
    }
}

impl ChannelReader {
    /// Blocks for the next chunk of bytes, racing against `self.control` (if any) the
    /// same way `TableOutputStream::send`/`InterruptibleTableInputStream::read` already
    /// race their own data channel against a control one: `Pause` blocks right here,
    /// looping on the control channel alone, until `Resume` or `Terminate` arrives;
    /// `Terminate` ends the read now instead of leaving it blocked forever on a producer
    /// that may never send anything.
    fn recv_next(&self) -> Result<Box<[u8]>, RecvFailure> {
        let Some(control) = &self.control else {
            return self.receiver.recv().map_err(|_| RecvFailure::Disconnected);
        };
        select! {
            recv(self.receiver) -> r => r.map_err(|_| RecvFailure::Disconnected),
            recv(control) -> msg => match msg {
                Ok(StreamControlMessage::Terminate) => Err(RecvFailure::Interrupted(terminate::<()>().unwrap_err())),
                Ok(StreamControlMessage::Resume) => self.recv_next(),
                Ok(StreamControlMessage::Pause) => {
                    loop {
                        match control.recv() {
                            Ok(StreamControlMessage::Terminate) => {
                                return Err(RecvFailure::Interrupted(terminate::<()>().unwrap_err()));
                            }
                            Ok(StreamControlMessage::Resume) => break,
                            Ok(StreamControlMessage::Pause) => {}
                            Err(e) => return Err(RecvFailure::Interrupted(e.into())),
                        }
                    }
                    self.recv_next()
                }
                Err(e) => Err(RecvFailure::Interrupted(e.into())),
            },
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, mut dst: &mut [u8]) -> Result<usize, Error> {
        match &self.buff {
            None => match self.recv_next() {
                Ok(b) => {
                    if b.len() == 0 {
                        self.read(dst)
                    } else {
                        self.buff = Some(b);
                        self.read(dst)
                    }
                }

                // A disconnected channel looks identical whether the producer finished
                // cleanly or failed partway through, after already having handed back
                // this reader (see GlobalState::recv_job_result's doc comment for why
                // that handoff can happen before the producer is actually done). If
                // tagged with the job still producing it, join every thread under that
                // job now -- by construction the channel only disconnects once they've
                // all actually exited, so this never blocks on anything that isn't
                // already finished -- and, if a real error turns up, smuggle it through
                // as an io::Error rather than reporting clean EOF (see this file's own
                // `Read` impl callers and CrushError's `From<std::io::Error>`, which
                // unwraps it back out). The tag is taken (cleared), not just read, so
                // every clone of this reader sees it resolved afterward too -- see the
                // `producer` field's own doc comment for why that sharing is essential.
                Err(RecvFailure::Disconnected) => match self.producer.lock().unwrap().take() {
                    Some((job, threads)) => match threads.join_job(job.id()) {
                        Ok(()) => Ok(0),
                        Err(real_err) => Err(Error::other(real_err)),
                    },
                    None => Ok(0),
                },

                // A `Terminate` control message (or, matching `TableOutputStream::send`'s
                // own choice, an unexpectedly disconnected control channel) -- smuggled
                // through as an io::Error the same way a real producer error already is,
                // just above.
                Err(RecvFailure::Interrupted(e)) => Err(Error::other(e)),
            },
            Some(src) => {
                if dst.len() >= src.len() {
                    let res = src.len();
                    dst.write_all(src)?;
                    self.buff = None;
                    Ok(res)
                } else {
                    let written = dst.write(src)?;
                    self.buff = Some(Box::from(&src[written..]));
                    Ok(written)
                }
            }
        }
    }
}

struct ChannelWriter {
    sender: Sender<Box<[u8]>>,
    /// See `BinaryWriter::register_control`'s doc comment.
    control: Option<Receiver<StreamControlMessage>>,
}

impl Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        let Some(control) = &self.control else {
            let _ = self.sender.send(buf.into());
            return Ok(buf.len());
        };
        // Races the (possibly blocking, on a full bounded(32) channel) send against
        // `control`, the same way TableOutputStream::send already races its own send --
        // Pause blocks right here, looping on the control channel alone, until Resume or
        // Terminate; Resume/a post-Pause Resume retries the whole write via a recursive
        // call rather than a manual loop, so `buf` (never consumed unless the send arm
        // itself is actually chosen) is simply reused as-is. Disconnection of the *data*
        // channel is left exactly as before -- silently ignored, matching a downstream
        // reader that's simply stopped reading early.
        select! {
            send(self.sender, buf.into()) -> res => {
                let _ = res;
                Ok(buf.len())
            }
            recv(control) -> msg => match msg {
                Ok(StreamControlMessage::Terminate) => Err(Error::other(terminate::<()>().unwrap_err())),
                Ok(StreamControlMessage::Resume) => self.write(buf),
                Ok(StreamControlMessage::Pause) => {
                    loop {
                        match control.recv() {
                            Ok(StreamControlMessage::Terminate) => {
                                return Err(Error::other(terminate::<()>().unwrap_err()));
                            }
                            Ok(StreamControlMessage::Resume) => break,
                            Ok(StreamControlMessage::Pause) => {}
                            Err(e) => return Err(Error::other(CrushError::from(e))),
                        }
                    }
                    self.write(buf)
                }
                Err(e) => Err(Error::other(CrushError::from(e))),
            },
        }
    }

    fn flush(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/// A writer that can be handed back as a binary_stream's producer, with an optional
/// job-control hookup -- the writer-side counterpart of `BinaryReader`. Every existing
/// caller of `binary_channel`/`files::writer` used to get a plain `Box<dyn Write>`; this
/// narrow, `Write`-extending trait lets a channel-backed writer additionally respond to
/// `crush:pause`/`crush:terminate` without changing how any of those callers use it (all
/// the `Write` methods stay directly callable through the supertrait bound).
pub trait BinaryWriter: Write + Send {
    /// Registers the job-control channel a paused/terminated job's `crush:pause`/
    /// `crush:terminate` sends `StreamControlMessage`s through, so a `write()` blocked
    /// because nothing is draining this writer's channel can be interrupted instead of
    /// hanging forever -- see `BinaryReader::register_control`'s identical doc comment
    /// for the read-side counterpart. A plain file has no reader to wait on and just
    /// ignores this.
    fn register_control(&mut self, _control: Receiver<StreamControlMessage>) {}
}

impl BinaryWriter for ChannelWriter {
    fn register_control(&mut self, control: Receiver<StreamControlMessage>) {
        self.control = Some(control);
    }
}

impl BinaryWriter for File {}

pub trait BinaryReader: Read + Debug + Send + Sync {
    fn clone(&self) -> Box<dyn BinaryReader + Send + Sync>;

    /// Tags this reader with the job still producing its bytes, so that once its
    /// underlying source disconnects, a real trailing error can be recovered instead of
    /// silently being reported as clean EOF -- see `ChannelReader`'s own `Read` impl.
    /// Most readers (a plain file, an in-memory buffer, ...) have no such notion of an
    /// in-flight producer and just ignore this.
    fn set_producer_job(&mut self, _job: JobHandle, _threads: ThreadStore) {}

    /// Registers the job-control channel a paused/terminated job's `crush:pause`/
    /// `crush:terminate` sends `StreamControlMessage`s through, so a `read()` blocked
    /// waiting on this reader's own producer can be interrupted instead of hanging
    /// forever -- mirrors `TableOutputStream`'s existing `control` field and
    /// `InterruptibleTableInputStream`'s existing `select!` against it. Most readers (a
    /// plain file, an in-memory buffer, ...) can never block waiting on a producer in
    /// the first place and just ignore this; only a channel-backed reader needs it.
    fn register_control(&mut self, _control: Receiver<StreamControlMessage>) {}
}

pub struct FileReader {
    file: File,
}

impl FileReader {
    pub fn new(file: File) -> FileReader {
        FileReader { file }
    }
}

impl Debug for FileReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("<file reader>")
    }
}

impl Read for FileReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.file.read(buf)
    }
}

impl BinaryReader for FileReader {
    fn clone(&self) -> Box<dyn BinaryReader + Send + Sync> {
        Box::from(FileReader {
            file: self.file.try_clone().unwrap(),
        })
    }
}

impl dyn BinaryReader {
    pub fn paths(mut files: Vec<PathBuf>) -> CrushResult<Box<dyn BinaryReader + Send + Sync>> {
        if files.len() == 1 {
            Ok(Box::from(FileReader::new(File::open(files.remove(0))?)))
        } else {
            let mut readers: Vec<Box<dyn BinaryReader + Send + Sync>> = Vec::new();

            for p in files.drain(..) {
                let f = Box::from(FileReader::new(File::open(p)?));
                readers.push(f);
            }
            Ok(Box::from(MultiReader {
                readers: VecDeque::from(readers),
            }))
        }
    }

    pub fn vec(bytes: &[u8]) -> Box<dyn BinaryReader + Send + Sync> {
        Box::from(BinaryVecReader {
            vec: Vec::from(bytes),
            offset: 0,
        })
    }
}

pub fn binary_channel() -> (Box<dyn BinaryWriter>, Box<dyn BinaryReader + Send + Sync>) {
    let (s, r) = bounded(32);
    (
        Box::from(ChannelWriter { sender: s, control: None }),
        Box::from(ChannelReader {
            receiver: r,
            buff: None,
            producer: Arc::new(Mutex::new(None)),
            control: None,
        }),
    )
}

pub(crate) struct MultiReader {
    readers: VecDeque<Box<dyn BinaryReader + Send + Sync>>,
}

impl MultiReader {
    pub fn new(readers: VecDeque<Box<dyn BinaryReader + Send + Sync>>) -> MultiReader {
        MultiReader { readers }
    }
}

impl BinaryReader for MultiReader {
    fn clone(&self) -> Box<dyn BinaryReader + Send + Sync> {
        let vec = self
            .readers
            .iter()
            .map(|r| r.as_ref().clone())
            .collect::<Vec<Box<dyn BinaryReader + Send + Sync>>>();
        Box::from(MultiReader {
            readers: VecDeque::from(vec),
        })
    }
}

impl Read for MultiReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        if self.readers.len() == 0 {
            return Ok(0);
        }
        match self.readers[0].read(buf) {
            Ok(0) => {
                self.readers.pop_front();
                self.read(buf)
            }
            Ok(s) => Ok(s),
            Err(e) => Err(e),
        }
    }
}

impl Debug for MultiReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("<multi reader>") //.map_err(|e| std::fmt::Error::default())
    }
}

struct BinaryVecReader {
    vec: Vec<u8>,
    offset: usize,
}

impl BinaryReader for BinaryVecReader {
    fn clone(&self) -> Box<dyn BinaryReader + Send + Sync> {
        Box::new(BinaryVecReader {
            vec: self.vec.clone(),
            offset: 0,
        })
    }
}

impl Read for BinaryVecReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let len = min(buf.len(), self.vec.len() - self.offset);
        buf[0..len].copy_from_slice(&self.vec[self.offset..self.offset + len]);
        self.offset += len;
        Ok(len)
    }
}

impl Debug for BinaryVecReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("<vec reader>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam::channel::unbounded;
    use std::time::Duration;

    // A blocked ChannelReader::read() must respect a Terminate control message the way
    // TableOutputStream::send/InterruptibleTableInputStream::read already do -- otherwise
    // a job stuck reading a binary_stream whose producer never sends anything (or hangs)
    // can never be interrupted via crush:terminate, only killed at the process level.
    // Run on a background thread with a bounded wait, not directly: a real hang here
    // must fail this test with a clear message, not hang the whole test binary.
    #[test]
    fn channel_reader_read_is_interrupted_by_terminate() {
        let (_writer, mut reader) = binary_channel();
        // Never write anything to _writer -- reader.read() would block on the channel
        // forever without a working control message.
        let (control_sender, control_receiver) = unbounded();
        reader.register_control(control_receiver);

        let (done_tx, done_rx) = crossbeam::channel::bounded(1);
        std::thread::spawn(move || {
            let mut buf = [0u8; 16];
            let result = reader.read(&mut buf);
            let _ = done_tx.send(result.is_err());
        });

        // Give the spawned thread a moment to actually reach the blocking recv().
        std::thread::sleep(Duration::from_millis(100));
        control_sender.send(StreamControlMessage::Terminate).expect(
            "control_receiver was already dropped -- register_control() isn't keeping it alive",
        );

        match done_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(was_err) => assert!(
                was_err,
                "a read() interrupted by Terminate should return an error, not Ok"
            ),
            Err(_) => panic!(
                "ChannelReader::read() did not respect a Terminate control message within 5s -- it's still blocked on the channel"
            ),
        }
    }

    // Writer-side counterpart of the read test above: a ChannelWriter::write() blocked
    // because nothing is draining the (bounded) channel must also respect Terminate,
    // not just hang until the process is killed. Fill the channel's own bounded(32)
    // capacity first so the write under test is provably blocked on backpressure, not
    // merely slow.
    #[test]
    fn channel_writer_write_is_interrupted_by_terminate() {
        let (sender, _receiver) = bounded(32);
        let mut writer = ChannelWriter { sender: sender.clone(), control: None };
        for _ in 0..32 {
            sender.send(Box::from([0u8])).expect("failed to pre-fill the bounded channel");
        }
        // _receiver is kept alive (never dropped) but never drained, so the 33rd send
        // below genuinely blocks on backpressure rather than on a disconnected channel.

        let (control_sender, control_receiver) = unbounded();
        writer.register_control(control_receiver);

        let (done_tx, done_rx) = crossbeam::channel::bounded(1);
        std::thread::spawn(move || {
            let result = writer.write(&[0u8]);
            let _ = done_tx.send(result.is_err());
        });

        // Give the spawned thread a moment to actually reach the blocking send().
        std::thread::sleep(Duration::from_millis(100));
        control_sender.send(StreamControlMessage::Terminate).expect(
            "control_receiver was already dropped -- register_control() isn't keeping it alive",
        );

        match done_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(was_err) => assert!(
                was_err,
                "a write() interrupted by Terminate should return an error, not Ok"
            ),
            Err(_) => panic!(
                "ChannelWriter::write() did not respect a Terminate control message within 5s -- it's still blocked on the channel"
            ),
        }
    }
}
