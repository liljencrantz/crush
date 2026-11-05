use crate::lang::errors::CrushResult;
use crate::lang::job_control::{
    ChannelBasedController, InterruptibleJoinHandle, StreamControlMessage,
};
use crate::lang::printer::Printer;
use crate::lang::state::handles::CommandHandle;
use crate::lang::state::id::{CommandId, JobId};
use chrono::{DateTime, Local};
use crossbeam::channel::unbounded;
use crossbeam::channel::bounded;
use itertools::Either;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::ThreadId;

/// A per-spawned-thread receipt, handed back by `ThreadStore::spawn`.
///
/// While this is alive, `ThreadStore::reap`'s global sweep leaves the thread alone even
/// once it's finished -- it's the caller's job to join it (directly, by id/job-id, or
/// indirectly by tucking this handle into a stream value's producer tag for a later
/// drain to join). Dropping it marks the thread's `ThreadData` entry `zombie`, which is
/// what actually orphans it: only then is `reap` free to join it on its next pass and
/// report any error directly, the same way it always has for a background job nobody
/// ever came back to check on.
///
/// Deliberately not `Clone`: nothing in this codebase ever needs two outstanding claims
/// on the same thread at once -- `spawn` hands out exactly one of these, and every
/// consumer (a direct join, the `Vec` collected across a job's stages, a stream's
/// producer tag) moves it rather than copies it, all the way through to the single
/// point where it's finally joined or simply allowed to drop. That single owner is what
/// lets a plain `bool`, flipped on that one `Drop`, stand in for what would otherwise
/// need an `Arc`/`Weak` refcount.
///
/// This is what actually closes the race `try_join_job`'s own doc comment describes
/// `reap` as having caused before it was scoped to a specific job id: that scoping
/// stopped `try_join_job`/`join_job` calls from stealing each *other's* threads, but
/// did nothing about the plain, unscoped `reap` sweep (still called from every new
/// job's own creation) racing a *deferred* claim -- a stream value that hasn't been
/// drained yet -- which is exactly the gap this closes instead.
pub struct ThreadHandle {
    id: ThreadId,
    store: ThreadStore,
}

impl ThreadHandle {
    pub fn id(&self) -> ThreadId {
        self.id
    }
}

impl Drop for ThreadHandle {
    fn drop(&mut self) {
        self.store.mark_orphaned(self.id);
    }
}

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
    /// Set once this thread's `ThreadHandle` has been dropped (see that type's own doc
    /// comment). `reap`'s global sweep only ever touches a thread once this is true.
    zombie: bool,
}

struct ThreadStoreInternal {
    threads: Vec<ThreadData>,
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
        ThreadStore {
            data: Arc::from(Mutex::new(ThreadStoreInternal {
                threads: Vec::new(),
            })),
        }
    }

    /**
    Spawn a new thread
    */
    pub fn spawn<F>(&self, name: &str, command: &CommandHandle, f: F) -> CrushResult<ThreadHandle>
    where
        F: FnOnce() -> CrushResult<()>,
        F: Send + 'static,
    {
        let (control_sender, control_receiver) = unbounded();
        let (result_sender, result_receiver) = bounded(1);

        command.register(Arc::from(ChannelBasedController::new(control_sender)));

        let handle = thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                let res = f();
                let _ = result_sender.send(res);
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
            zombie: false,
        });
        Ok(ThreadHandle {
            id,
            store: self.clone(),
        })
    }

    /// Mark `id`'s thread as orphaned -- called exactly once, from `ThreadHandle`'s own
    /// `Drop` impl. A no-op if the thread has already been removed from `self.data`
    /// (joined directly by id elsewhere, or already reaped): a `ThreadHandle` dropping
    /// after its thread was already joined isn't an error, just the common case.
    fn mark_orphaned(&self, id: ThreadId) {
        let mut data = self.data.lock().unwrap();
        if let Some(t) = data.threads.iter_mut().find(|t| t.handle.id() == id) {
            t.zombie = true;
        }
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
    Error report every *orphaned* thread that has already exited -- one whose
    `ThreadHandle` has already been dropped, marking it `zombie` (see that type's own
    doc comment). A thread whose handle is still held is left alone even if it has
    already finished: something
    else -- a direct `join_job`/`try_join_job` call, or a stream value still waiting to
    be drained -- is specifically responsible for it, and stealing its result out from
    under that would silently turn a real failure into a plain, swallowed disconnection
    (confirmed directly: this is exactly what used to happen to `try`/`catch` around a
    stream-producing command whose failure is only discoverable once its output is
    actually read, e.g. `uniq` on an unhashable column -- a concurrently created,
    entirely unrelated job's own routine `reap` call could win the race to join the
    failing thread first, printing its error here instead of letting it reach the
    `catch` clause waiting for it).
    */
    pub fn reap(&self, printer: &Printer) {
        let ids: Vec<ThreadId> = {
            let data = self.data.lock().unwrap();
            data.threads
                .iter()
                .filter(|t| t.zombie)
                .map(|t| t.handle.id())
                .collect()
        };
        for id in ids {
            if let Some(result) = self.try_reap_one(id) {
                printer.handle_error(result);
            }
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
                    StreamControlMessage::Resume | StreamControlMessage::ResumeForeground => {}
                },
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    /// Like `try_join_one`, but used only by `reap`'s own unscoped sweep: never touches
    /// the control channel at all, only the thread's real result (see
    /// `InterruptibleJoinHandle::try_recv_result`'s own doc comment for why that
    /// distinction matters here specifically -- `try_join_one`'s destructive peek at a
    /// stale `Terminate`/`Resume` message is correct for a caller specifically
    /// interested in this one thread, but would silently and permanently throw that
    /// message away for a thread `reap` is only speculatively checking on, one that may
    /// never produce a real result at all).
    fn try_reap_one(&self, id: ThreadId) -> Option<CrushResult<()>> {
        let mut data = self.data.lock().unwrap();
        let idx = (0..data.threads.len()).find(|&idx| data.threads[idx].handle.id() == id)?;
        let result = data.threads[idx].handle.try_recv_result()?;
        data.threads.remove(idx);
        Some(result)
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
