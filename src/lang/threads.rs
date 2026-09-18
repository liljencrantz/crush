use crate::lang::errors::CrushResult;
use crate::lang::job_control::{
    ChannelBasedController, InterruptibleJoinHandle, StreamControlMessage,
};
use crate::lang::printer::Printer;
use crate::lang::state::handles::CommandHandle;
use crate::lang::state::id::{CommandId, JobId};
use chrono::{DateTime, Local};
use crossbeam::channel::Sender;
use crossbeam::channel::unbounded;
use crossbeam::channel::{Receiver, bounded};
use itertools::Either;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::ThreadId;
use std::time::Duration;

/**
A thread management utility. Spawn, track and join on threads.
*/
#[allow(dead_code)] // Command is never read but is needed for resource tracking
struct ThreadData {
    handle: InterruptibleJoinHandle<CrushResult<()>>,
    creation_time: DateTime<Local>,
    command: CommandHandle,
    job_id: JobId,
    command_id: CommandId,
}

struct ThreadStoreInternal {
    threads: Vec<ThreadData>,
    sender: Sender<ThreadId>,
    receiver: Receiver<ThreadId>,
}

pub struct ThreadDescription {
    pub name: String,
    pub creation_time: DateTime<Local>,
    pub job_id: JobId,
    pub command_id: CommandId,
}

#[derive(Clone)]
pub struct ThreadStore {
    data: Arc<Mutex<ThreadStoreInternal>>,
}

impl ThreadStore {
    pub fn new() -> ThreadStore {
        let (sender, receiver) = unbounded();

        ThreadStore {
            data: Arc::from(Mutex::new(ThreadStoreInternal {
                threads: Vec::new(),
                sender,
                receiver,
            })),
        }
    }

    fn exit(&self) {
        let data = self.data.lock().unwrap();
        let _ = data.sender.send(std::thread::current().id());
    }

    /**
    Spawn a new thread
    */
    pub fn spawn<F>(&self, name: &str, command: &CommandHandle, f: F) -> CrushResult<ThreadId>
    where
        F: FnOnce() -> CrushResult<()>,
        F: Send + 'static,
    {
        let slef = self.clone();

        let (control_sender, control_receiver) = unbounded();
        let (result_sender, result_receiver) = bounded(1);

        command.register(Box::from(ChannelBasedController::new(control_sender)));

        let handle = thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                let res = f();
                slef.exit();
                result_sender.send(res)
            })?;
        let id = handle.thread().id();
        let handle2 = InterruptibleJoinHandle::new(
            id,
            handle.thread().name(),
            result_receiver,
            control_receiver,
        );
        let mut data = self.data.lock().unwrap();
        data.threads.push(ThreadData {
            handle: handle2,
            creation_time: Local::now(),
            command: command.clone(),
            job_id: command.job_handle.id(),
            command_id: command.id,
        });
        Ok(id)
    }

    /**
    Block calling thread until all other threads have exited
    */
    pub fn join(&self, printer: &Printer) {
        loop {
            let mut data = self.data.lock().unwrap();
            match data.threads.pop() {
                None => break,
                Some(h) => {
                    drop(data);
                    printer.handle_error(h.handle.join());
                }
            }
        }
    }

    /**
    Error report all threads that have already exited
    */
    pub fn reap(&self, printer: &Printer) {
        let data = self.data.lock().unwrap();
        let mut kill_list = Vec::new();
        while let Ok(id) = data.receiver.recv_timeout(Duration::from_nanos(0)) {
            kill_list.push(id);
        }
        drop(data);
        for id in kill_list {
            printer.handle_error(self.join_one(id));
        }
    }

    /**
    Block calling thread until specified thread has exited. Returns the command's own
    result, so that a failing command actually propagates to the caller instead of only
    ever being printed and discarded here.
    */
    pub fn join_one(&self, id: ThreadId) -> CrushResult<()> {
        let mut data = self.data.lock().unwrap();
        let mut thread_idx = None;
        for idx in 0..data.threads.len() {
            if data.threads[idx].handle.id() == id {
                thread_idx = Some(idx);
                break;
            }
        }
        if let Some(idx) = thread_idx {
            let h = data.threads.remove(idx);
            drop(data);

            match h.handle.join() {
                Ok(Either::Left(res)) => return res,
                Ok(Either::Right(m)) => match m {
                    StreamControlMessage::Terminate => {}
                    StreamControlMessage::Pause => {
                        let mut data = self.data.lock().unwrap();
                        data.threads.push(h);
                    }
                    StreamControlMessage::Resume => {}
                },
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    /// Non-blocking: if `id`'s thread has *already* finished (a real result, not just a
    /// job-control message), removes and returns its result; a `Pause` control message
    /// is handled exactly as `join_one` handles it (left registered, for the same
    /// pause/resume machinery to find later) and, like anything still running, reported
    /// back as `None`. Never blocks and never touches any thread other than `id`.
    fn try_join_one(&self, id: ThreadId) -> Option<CrushResult<()>> {
        let mut data = self.data.lock().unwrap();
        let idx = (0..data.threads.len()).find(|&idx| data.threads[idx].handle.id() == id)?;
        match data.threads[idx].handle.try_join() {
            None => None,
            Some(Ok(Either::Left(res))) => {
                data.threads.remove(idx);
                Some(res)
            }
            Some(Ok(Either::Right(StreamControlMessage::Pause))) => None,
            Some(Ok(Either::Right(_))) => None,
            Some(Err(err)) => {
                data.threads.remove(idx);
                Some(Err(err))
            }
        }
    }

    fn thread_ids_for_job(&self, job_id: JobId) -> Vec<ThreadId> {
        let data = self.data.lock().unwrap();
        data.threads
            .iter()
            .filter(|t| t.job_id == job_id)
            .map(|t| t.handle.id())
            .collect()
    }

    /// Block the calling thread until every thread currently tracked under `job_id` has
    /// exited. There's no need to keep watching for new ones to appear here: by the
    /// time a caller has a reason to call this, `job_id`'s own `Job::eval` has already
    /// returned, so no further thread can still be spawned under it. Every one of them
    /// still gets joined even after finding a real error, so none of them leak, but
    /// only the first real (non-benign-`SendError`) error found is returned -- the same
    /// single-error contract `join_one` has for one thread, just widened to cover every
    /// stage of a job (a 5-stage pipeline can have up to 5 of these) instead of one
    /// specific thread.
    pub fn join_job(&self, job_id: JobId) -> CrushResult<()> {
        let mut first_error = None;
        for id in self.thread_ids_for_job(job_id) {
            if let Err(e) = self.join_one(id) {
                if !e.is_send_disconnected() && first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }
        match first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Like `join_job`, but never blocks: only touches threads under `job_id` that have
    /// *already* finished, leaving any still-running ones exactly as they were for a
    /// later `join_job`/`try_join_job` call to find. Safe to call speculatively on a job
    /// another caller might still be responsible for: unlike a broader sweep (there is
    /// deliberately no "reap whatever else has exited too, for any job" variant of
    /// this), it can never remove a thread under a *different* job_id out from under a
    /// caller who's about to `join_job` it for a real error -- which is exactly what
    /// made an earlier version of this cleanup (a plain global `reap()` call in the same
    /// spots) unsafe: `reap()` has no way to leave a specific still-wanted job's threads
    /// alone, so it could -- and demonstrably did -- silently swallow a real command
    /// failure by printing (rather than propagating) whichever thread it happened to
    /// reap first, including ones a concurrently-running `join_job`/`join_one` was about
    /// to legitimately claim.
    pub fn try_join_job(&self, job_id: JobId) -> CrushResult<()> {
        let mut first_error = None;
        for id in self.thread_ids_for_job(job_id) {
            if let Some(Err(e)) = self.try_join_one(id) {
                if !e.is_send_disconnected() && first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }
        match first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    pub fn current_threads(&self) -> CrushResult<Vec<ThreadDescription>> {
        let data = self.data.lock().unwrap();
        let res = Ok(data
            .threads
            .iter()
            .map(|t| ThreadDescription {
                name: t.handle.name().clone().unwrap_or("<unnamed>".to_string()),
                creation_time: t.creation_time.clone(),
                job_id: t.job_id,
                command_id: t.command_id,
            })
            .collect());
        res
    }
}
