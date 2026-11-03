use crate::lang::errors::CrushResult;
use crate::lang::job_control::{JobController, StreamControlMessage};
use crate::lang::state::id::CommandId;
use crate::lang::state::id::JobId;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt::Display;
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone)]
pub struct JobHandle {
    id: JobId,
    live_job: Arc<Mutex<JobControlData>>,
}

impl JobHandle {
    pub fn new(id: JobId) -> JobHandle {
        JobHandle {
            id,
            live_job: Arc::from(Mutex::from(JobControlData {
                definition: String::new(),
                senders: HashMap::new(),
                next_command_id: CommandId::first(),
                status: JobStatus::Running,
                termination_requested: false,
            })),
        }
    }

    pub fn from(id: JobId, live_job: Arc<Mutex<JobControlData>>) -> JobHandle {
        JobHandle { id, live_job }
    }

    pub fn set_name(&self, name: impl Into<String>) {
        let mut live_job = self.live_job.lock().unwrap();
        live_job.definition = name.into();
    }

    pub fn command_handle(&self, id: CommandId) -> CommandHandle {
        CommandHandle {
            job_handle: self.clone(),
            id,
        }
    }

    pub fn next_command_handle(&self) -> CommandHandle {
        let mut live_job = self.live_job.lock().unwrap();
        let id = live_job.next_command_id;
        live_job.next_command_id.next();
        CommandHandle {
            job_handle: self.clone(),
            id,
        }
    }

    pub fn current_command_handle(&self) -> CommandHandle {
        let live_job = self.live_job.lock().unwrap();
        let id = live_job.next_command_id;
        CommandHandle {
            job_handle: self.clone(),
            id,
        }
    }

    /// Registers `controller` for `command_handle`, then replays the job's
    /// already-settled state to it if one exists.
    ///
    /// Closes the registration race between `ThreadStore::spawn`'s generic controller
    /// (registered on the *calling* thread before the worker thread is even created,
    /// so inherently race-free) and the second, real controller several builtins
    /// (`cmd`, `sleep`, `fs:watch`, `schedule`, `remote:pexec`, and
    /// `CommandContext::initialize_output`, used by every stream-source builtin)
    /// register from *inside* that worker thread, once the OS actually schedules it --
    /// which can be arbitrarily later. Before this, a `terminate`/`pause` landing in
    /// that window reached only the inert generic controller; the real controller,
    /// not existing in `senders` yet, never found out, so the real work (killing an
    /// external process, breaking out of `sleep`) silently never happened.
    ///
    /// Gated on `termination_requested`, not raw `status == Terminated`: `status` also
    /// reads `Terminated` after `mark_terminated` (an external process dying from an
    /// unrelated signal, bookkeeping only -- see its own doc comment for why that must
    /// never broadcast a real `StreamControlMessage::Terminate`), and
    /// `termination_requested` stays true forever once an explicit terminate actually
    /// happens even if the job is later resumed (see that field's own doc comment) --
    /// so the live `status` is still checked too, to avoid replaying a stale terminate
    /// to a controller for a job that is, right now, genuinely running again.
    ///
    /// The replay call itself -- like every `JobController` call, see that type's own
    /// doc comment -- happens *after* `live_job`'s lock is dropped, not while still
    /// holding it: `controller`'s method can block (a `ChannelBasedController` sending
    /// into a bounded channel nobody is draining yet), and this job's lock is shared
    /// with every other command under it still trying to register or look up state --
    /// holding it through a block here would stop all of them too, for as long as the
    /// block lasts, which can be forever.
    pub fn register_command(&self, command_handle: &CommandHandle, controller: JobController) {
        let replay = {
            let mut live_job = self.live_job.lock().unwrap();
            match live_job.senders.entry(command_handle.id) {
                Entry::Occupied(mut e) => e.get_mut().push(controller.clone()),
                Entry::Vacant(e) => {
                    e.insert(vec![controller.clone()]);
                }
            }
            if live_job.termination_requested && live_job.status == JobStatus::Terminated {
                Some(StreamControlMessage::Terminate)
            } else if live_job.status == JobStatus::Paused {
                Some(StreamControlMessage::Pause)
            } else {
                None
            }
        };
        if let Some(message) = replay {
            let _ = match message {
                StreamControlMessage::Terminate => controller.terminate(),
                StreamControlMessage::Pause => controller.pause(),
                StreamControlMessage::Resume | StreamControlMessage::ResumeForeground => Ok(()),
            };
        }
    }

    pub fn unregister_command(&self, id: &CommandHandle) {
        let mut live_job = self.live_job.lock().unwrap();
        live_job.senders.remove(&id.id);
    }

    pub fn id(&self) -> JobId {
        self.id
    }

    pub fn weak_ref(&self) -> Weak<Mutex<JobControlData>> {
        Arc::downgrade(&self.live_job)
    }

    pub fn definition(&self) -> String {
        let live_job = self.live_job.lock().unwrap();
        live_job.definition.clone()
    }

    /// True if `terminate` has *actually been explicitly requested* on this job --
    /// deliberately not the same question as "does this job's live status currently
    /// read `Terminated`" (see `JobControlData::termination_requested`'s own doc
    /// comment for why conflating the two is a real bug, not just imprecise naming:
    /// `status` also reads `Terminated` after `mark_terminated`, used for an external
    /// `cmd` job's child dying from an unrelated OS signal -- a case that must *not* be
    /// treated as an explicit termination request).
    ///
    /// `GlobalState::terminate` sets this *synchronously*, inside the same call that
    /// broadcasts a `StreamControlMessage::Terminate` to this job's registered
    /// controllers -- independent of whether any of those controllers' own command
    /// threads ever actually get joined, let alone whether a specific join call
    /// happens to observe their real result. Checking this flag directly (rather than
    /// relying on a join's return value to infer "was this terminated") sidesteps two
    /// independent races that can otherwise silently discard a terminated statement's
    /// real `Err` result, confirmed directly via `{sleep $(duration:of seconds=100);
    /// echo SHOULD_NOT_APPEAR} &` followed by `terminate` on the whole block -- see
    /// `tests/terminate_mid_closure_statement_stops_next_statement.crush`:
    ///
    /// 1. **`ThreadStore::reap` racing a deliberate `join_job`/`join_one` call.**
    ///    `ThreadStore::join_job` first collects the ids of every thread currently
    ///    registered under a job, then joins each separately -- and in the gap between
    ///    those two steps, `ThreadStore::reap` (called by *any* new job being created
    ///    anywhere in the system, not just the one actually waiting on this result) can
    ///    win the race to remove and join that same thread first, leaving the original
    ///    `join_job` call with nothing left to find for that thread id (treated the
    ///    same as "that thread already finished cleanly"). This was the mechanism
    ///    actually observed causing the failure above, confirmed via targeted tracing.
    /// 2. **The generic wrapper controller racing the command's own result, even with
    ///    no `reap` involved at all.** `ThreadStore::spawn` always registers its own
    ///    generic `ChannelBasedController` under the same `CommandId` a command like
    ///    `sleep` separately self-registers under, so `GlobalState::terminate`'s
    ///    broadcast loop sends a `StreamControlMessage::Terminate` to *both*. Even when
    ///    a single caller legitimately owns the join with nobody else racing it,
    ///    `InterruptibleJoinHandle::join`'s own `select!` between the thread's real
    ///    result and that generic wrapper's raw control message doesn't prefer the
    ///    result when both happen to already be ready -- and `ThreadStore::join_one`
    ///    treats a raw `Right(Terminate)` the same as "no error" (`StreamControlMessage
    ///    ::Terminate => {}`), discarding the command's real `Err` just the same. This
    ///    path doesn't need any other thread/job in the system to trigger it.
    ///
    /// Both races only ever matter once a termination was genuinely requested in the
    /// first place, so reading this flag directly -- set unconditionally and
    /// synchronously the moment that happens -- is race-free regardless of which of the
    /// two (or whether either) actually fired on any given run.
    pub fn termination_requested(&self) -> bool {
        let live_job = self.live_job.lock().unwrap();
        live_job.termination_requested()
    }
}

#[derive(Clone)]
pub struct CommandHandle {
    pub job_handle: JobHandle,
    pub id: CommandId,
}

impl CommandHandle {
    pub fn register(&self, controller: JobController) {
        self.job_handle.register_command(self, controller);
    }

    pub fn unregister(&self) {
        self.job_handle.unregister_command(self);
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum JobStatus {
    Running,
    Paused,
    Terminated,
}

impl Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            JobStatus::Running => "Running",
            JobStatus::Paused => "Paused",
            JobStatus::Terminated => "Terminated",
        })
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum JobType {
    Interactive,
    Background,
}

impl Display for JobType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            JobType::Interactive => "Interactive",
            JobType::Background => "Background",
        })
    }
}

pub struct JobInfo {
    pub id: JobId,
    pub job_type: JobType,
    /// The job's definition, i.e. the invocation that started it, as source text.
    pub definition: String,
    pub status: JobStatus,
    /// The job this job is nested inside (e.g. a closure/block body evaluated as part of
    /// an enclosing job), if any. Lets code that needs to know about "other" jobs (like
    /// `exit`'s running-jobs check) tell an ancestor apart from a genuinely
    /// unrelated, concurrently running job.
    pub parent: Option<JobId>,
}

pub struct JobControlData {
    /// The job's definition, i.e. the invocation that started it, as source text.
    pub definition: String,
    status: JobStatus,
    /// True once `terminate` (not `mark_terminated`) has been called on this job --
    /// i.e. an *explicit* termination was actually requested, as opposed to `status`
    /// merely reading `Terminated` because `mark_terminated` recorded an external
    /// process dying from some unrelated signal (see `mark_terminated`'s own doc
    /// comment). `status == Terminated` alone can't be used to tell these apart, and
    /// conflating them is a real regression, not a hypothetical one: a backgrounded
    /// closure containing a `cmd` that dies from an ordinary, unrelated signal (e.g.
    /// `cmd sh -c "kill -TERM $$"`) must keep running its closure's remaining
    /// statements exactly as before -- see `tests/cmd_signal_death_does_not_abort_
    /// script.crush` for why that case must *not* abort anything, and
    /// `tests/signal_death_inside_background_closure_does_not_abort.crush` for the
    /// regression this field exists to prevent. See `JobHandle::termination_requested`
    /// for where this is actually read.
    ///
    /// Unlike `status`, never reset by `pause`/`resume`/`resume_foreground` -- once an
    /// explicit `terminate` has actually been requested, it stays requested for this
    /// job's whole remaining lifetime, even if `status` itself later moves back to
    /// `Running`/`Paused` (e.g. `resume` on a job that was marked `Terminated` but
    /// never actually died -- see `tests/resume_after_terminate_shows_current_status.
    /// crush`). This is intentional: "was termination ever requested" is a strictly
    /// different, one-way question from "what does `status` currently read", and
    /// nothing resuming a job changes the fact that someone did ask for it to stop.
    termination_requested: bool,
    senders: HashMap<CommandId, Vec<JobController>>,
    next_command_id: CommandId,
}

pub struct JobData {
    pub id: JobId,
    pub job_type: JobType,
    pub parent: Option<JobId>,
    pub job_control_data: Weak<Mutex<JobControlData>>,
    /// Captured synchronously by `GlobalState::terminate` at the moment it calls
    /// `JobControlData::terminate` on this job -- while `job_control_data` is still
    /// guaranteed alive, since locating it in the first place required
    /// `job_control_data.upgrade()` to have just succeeded.
    ///
    /// This exists because, for an external `cmd`-spawned job, the gap between "marked
    /// Terminated" and "the job's own thread has fully finished and dropped
    /// `job_control_data`" can be microseconds (SIGKILL is near-instant) -- easily short
    /// enough that by the time *any* subsequent `jobs` command runs, `job_control_data`
    /// has already died and its live `status()` is gone forever. Every `jobs` invocation
    /// is itself a brand new job, and creating a new job always reaps already-finished
    /// ones *before* building its own listing (see `GlobalState::create_job_handle_with_parent`),
    /// so without a durable snapshot taken at termination time, a terminated job could
    /// never actually be observed as `Terminated` -- it would just silently vanish, every
    /// single time, not merely as an occasional race.
    ///
    /// `Some(definition)` means this job was explicitly terminated (or, for an external
    /// `cmd` job, was observed to have died from a real OS signal -- see
    /// `GlobalState::mark_terminated_by_signal`) and hasn't been reported as
    /// `Terminated` by a `jobs` *listing* call yet (`GlobalState::jobs_for_listing`,
    /// used only by the `jobs` builtin -- never by `GlobalState::jobs`, which every
    /// other internal caller uses and which only ever reports genuinely live jobs).
    /// `jobs_for_listing` clears it back to `None` once reported, so the entry is
    /// eligible for actual removal on the next reap pass (see `remove_finished_jobs`)
    /// -- giving it exactly one listing, the same "reported once, then gone" shape a
    /// POSIX shell's own job control has.
    ///
    /// Only ever set for the one job actually terminated/killed, never for its
    /// descendants (see `GlobalState::terminate`) -- otherwise terminating a job with
    /// several nested closure jobs underneath it would produce one phantom row per
    /// descendant instead of just the one the user actually asked about.
    pub terminated_snapshot: Option<String>,
    /// How many `remove_finished_jobs` passes this entry has survived purely because of
    /// an unconsumed `terminated_snapshot` -- bounds the snapshot's lifetime (see
    /// `MAX_TERMINATED_SNAPSHOT_AGE` in `global_state.rs`) so a terminated job whose
    /// snapshot nobody ever consumes (no `jobs` call made afterward) doesn't pin this
    /// `JobData` entry, and its id, in `data.jobs` forever.
    pub terminated_snapshot_age: u32,
    /// Set the first time `GlobalState::jobs_for_listing` ever reports this job as
    /// `Terminated` -- via *either* `terminated_snapshot` or its live status happening
    /// to already read `Terminated`. While set, a later `jobs_for_listing` call skips
    /// this job *only as long as it's still observably `Terminated`* (still alive with
    /// that same live status, or already fully gone with nothing left to report) --
    /// it is cleared again, by `jobs_for_listing` itself, the moment live data shows
    /// this job's status has moved away from `Terminated` (e.g. `resume` on an
    /// internal job that was marked `Terminated` but never actually stopped), so it
    /// shows up again under its current status rather than staying hidden forever.
    /// It's also cleared by `snapshot` (see that function) whenever a fresh
    /// `terminate` call takes a new snapshot, so an explicit second `terminate` on the
    /// same job gets its own one-time report too.
    ///
    /// Exists because the one-time `terminated_snapshot` fallback alone isn't enough to
    /// guarantee "a given termination event is reported exactly once": once consumed,
    /// a later `jobs_for_listing` call falls through to the live branch, which can
    /// still legitimately read `Terminated` for a while (the underlying thread hasn't
    /// finished exiting and being reaped yet) -- or, for a job whose work never
    /// actually stops at all (e.g. an internal closure/loop that never checks its own
    /// control channel), indefinitely. Confirmed directly: two back-to-back `jobs`
    /// listings right after `terminate`, with no settle delay in between,
    /// intermittently both reported the job `Terminated` even with
    /// `mark_terminated_and_snapshot`'s own re-snapshot guard in place -- the second
    /// one via the live branch, not a second snapshot.
    pub terminated_reported: bool,
    /// True while this job is *not* holding the foreground -- directly, or because some
    /// job it's nested inside (however many levels deep) isn't. The single source of
    /// truth for that question; query it via `GlobalState::is_backgrounded`.
    ///
    /// Maintained at every transition, never derived later:
    /// - **Creation:** copied from the parent's own value
    ///   (`GlobalState::create_job_handle_with_parent`).
    /// - **Leaving the foreground** -- set to `true` on the job *and every live
    ///   descendant*, at exactly two places:
    ///   - a trailing `&`: the very top of `Job::eval`, before any stage dispatches;
    ///   - any pause: `GlobalState::pause`, the single choke point every pause goes
    ///     through (Ctrl-Z, the `pause` builtin, a kernel stop seen by `cmd`, a re-pause
    ///     while `fg` waits), before any controller hears about it. This matters even
    ///     when no `Job::eval` ever registers the paused job as a background job -- e.g.
    ///     Ctrl-Z on `{ { cmd sleep 100 } }` pauses only the innermost statement's job.
    ///   (`bg`/`resume` resume a job in place and leave it `true`.)
    /// - **Regaining the foreground** -- `fg` (`wait_for_background_job`) sets it on the
    ///   job and its live descendants to the `fg` caller's *own* value: the job now holds
    ///   exactly as much of the foreground as whoever is waiting on it.
    ///
    /// Baked into each job's own entry rather than discovered later by walking live
    /// `parent` links, because that walk doesn't survive the case that matters: the outer
    /// backgrounded job in a shape like `{ cmd sleep 100 } &` finishes its own dispatch
    /// and gets reaped almost immediately, and `remove_finished_jobs`'s orphaning logic
    /// then clears `parent` on the nested `cmd` job, long before the `sleep` dies. The
    /// descendant walks at the transitions above happen while the job being changed is
    /// alive by definition, and use the very same walk `pause`/`resume_foreground`
    /// themselves use to reach descendants, so "which jobs were paused/resumed" and
    /// "which jobs changed foreground status" always agree.
    pub backgrounded: bool,
}

/// Calls `terminate()` on every controller in `controllers`. Meant to run *after* the
/// caller has dropped whatever job-state lock produced this list -- see
/// `JobController`'s own doc comment for why calling a controller while still holding
/// that lock is unsafe (it can block, e.g. `ChannelBasedController` sending into a
/// bounded channel nobody is draining yet).
pub fn notify_terminate(controllers: &[JobController]) {
    for c in controllers {
        let _ = c.terminate();
    }
}

/// Like `notify_terminate`, but for `pause`.
pub fn notify_pause(controllers: &[JobController]) {
    for c in controllers {
        let _ = c.pause();
    }
}

/// Like `notify_terminate`, but for `resume`.
pub fn notify_resume(controllers: &[JobController]) {
    for c in controllers {
        let _ = c.resume();
    }
}

/// Like `notify_terminate`, but for `resume_foreground`.
pub fn notify_resume_foreground(controllers: &[JobController]) {
    for c in controllers {
        let _ = c.resume_foreground();
    }
}

impl JobControlData {
    pub fn status(&self) -> JobStatus {
        self.status
    }

    /// See `termination_requested`'s own doc comment.
    pub fn termination_requested(&self) -> bool {
        self.termination_requested
    }

    /// Updates this job's own state and returns every controller that must now be told
    /// to terminate -- it does *not* call them itself. See `JobController`'s own doc
    /// comment: a controller's `terminate()` can block, and this method runs while the
    /// caller holds this job's state lock (needed to update `status`/
    /// `termination_requested` atomically with reading `senders`), so the caller must
    /// drop that lock before actually calling `terminate()` on what's returned here.
    pub fn terminate(&mut self) -> CrushResult<Vec<JobController>> {
        let controllers = self.senders.values().flatten().cloned().collect();
        self.status = JobStatus::Terminated;
        self.termination_requested = true;
        Ok(controllers)
    }

    /// Like `terminate`, but *only* updates this job's own status -- it never notifies
    /// any registered controller via `StreamControlMessage::Terminate`. Used when a
    /// job's underlying process is already known to be dead (e.g. an external `cmd`
    /// job's child killed by a real OS signal sent directly to it, bypassing crush's
    /// own job control entirely -- see `GlobalState::mark_terminated_by_signal`), so
    /// there is nothing left to notify and no OS process left to (redundantly, and
    /// racily -- the pid could already have been reused by an unrelated process) signal
    /// again.
    ///
    /// Broadcasting a real `StreamControlMessage::Terminate` here would be actively
    /// wrong, not just redundant: every controller registered for this job -- including,
    /// for a `cmd` job, `wait_for_child_inner`'s own control-channel listener -- treats
    /// that message as "a termination was explicitly requested", which makes `cmd`
    /// return an error (aborting the whole script, not just this one command) even for
    /// an entirely ordinary signal-caused death (e.g. a pipeline stage killed by
    /// `SIGPIPE`, or a process that terminates itself with `kill -TERM $$`) that was
    /// never meant to be treated as a `terminate` request at all. Confirmed directly:
    /// routing a plain `SIGTERM`-killed `cmd sh -c 'kill -TERM $$'` through the full
    /// `terminate()` above made the enclosing script abort instead of continuing past
    /// it, exactly the regression this method exists to avoid.
    pub fn mark_terminated(&mut self) {
        self.status = JobStatus::Terminated;
    }

    /// Like `terminate`, but for `pause` -- see that method's own doc comment for why
    /// this returns the controllers to notify rather than notifying them itself.
    pub fn pause(&mut self) -> CrushResult<Vec<JobController>> {
        let controllers = self.senders.values().flatten().cloned().collect();
        self.status = JobStatus::Paused;
        Ok(controllers)
    }

    /// Like `terminate`, but for `resume` -- see that method's own doc comment for why
    /// this returns the controllers to notify rather than notifying them itself.
    pub fn resume(&mut self) -> CrushResult<Vec<JobController>> {
        let controllers = self.senders.values().flatten().cloned().collect();
        self.status = JobStatus::Running;
        Ok(controllers)
    }

    /// Like `resume`, but for a job being brought into the foreground (`fg`) rather
    /// than just unpaused in place (`bg`/`resume`) -- see
    /// `JobControl::resume_foreground`. Like `terminate`, returns the controllers to
    /// notify rather than notifying them itself.
    pub fn resume_foreground(&mut self) -> CrushResult<Vec<JobController>> {
        let controllers = self.senders.values().flatten().cloned().collect();
        self.status = JobStatus::Running;
        Ok(controllers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::job_control::{ChannelBasedController, JobControl};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A `JobController` spy: records how many times each `JobControl` method was
    /// called on it, so a test can assert a controller that registered *after* a
    /// terminate/pause already happened still actually heard about it.
    #[derive(Clone)]
    struct SpyController {
        terminate_calls: Arc<AtomicUsize>,
        pause_calls: Arc<AtomicUsize>,
    }

    impl SpyController {
        fn new() -> (Self, Arc<AtomicUsize>, Arc<AtomicUsize>) {
            let terminate_calls = Arc::new(AtomicUsize::new(0));
            let pause_calls = Arc::new(AtomicUsize::new(0));
            (
                SpyController {
                    terminate_calls: terminate_calls.clone(),
                    pause_calls: pause_calls.clone(),
                },
                terminate_calls,
                pause_calls,
            )
        }
    }

    impl JobControl for SpyController {
        fn terminate(&self) -> CrushResult<()> {
            self.terminate_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn pause(&self) -> CrushResult<()> {
            self.pause_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn resume(&self) -> CrushResult<()> {
            Ok(())
        }
        fn resume_foreground(&self) -> CrushResult<()> {
            Ok(())
        }
    }

    // Regression test for the registration race this plan's job-control audit found:
    // `ThreadStore::spawn` registers a generic controller for a command *before*
    // spawning its worker thread, but commands like `cmd`/`sleep`/`fs:watch`/
    // `schedule`/`remote:pexec` (and `CommandContext::initialize_output`, used by every
    // stream-source builtin) each register their own *second*, real controller only
    // once their already-spawned worker thread actually starts running -- which can be
    // arbitrarily later, purely depending on OS thread scheduling. A `terminate` sent
    // in that window previously reached only the inert generic controller (the real one
    // didn't exist in `senders` yet), so the real work (e.g. killing an external
    // process, breaking out of `sleep`) never happened at all.
    #[test]
    fn a_controller_registered_after_terminate_still_gets_terminated() {
        let handle = JobHandle::new(JobId::from(1));
        let command_handle = handle.next_command_handle();

        let (early, early_terminate_calls, _) = SpyController::new();
        command_handle.register(Arc::from(early));

        // Simulates `GlobalState::terminate` -- the real caller goes through more
        // machinery (locating the job, taking a snapshot) and, like here, notifies the
        // returned controllers only after dropping the lock (see `JobController`'s own
        // doc comment for why).
        let controllers = handle.live_job.lock().unwrap().terminate().unwrap();
        notify_terminate(&controllers);
        assert_eq!(
            early_terminate_calls.load(Ordering::SeqCst),
            1,
            "the already-registered controller must see the terminate immediately"
        );

        // The "real" controller a command like `cmd` registers from inside its own
        // worker thread, after the generic one already received the terminate above.
        let (late, late_terminate_calls, _) = SpyController::new();
        command_handle.register(Arc::from(late));

        assert_eq!(
            late_terminate_calls.load(Ordering::SeqCst),
            1,
            "a controller that registers after the job was already terminated must be \
             told immediately -- otherwise the real work it's about to do (e.g. `cmd` \
             spawning an external process, `sleep` starting its wait) never learns the \
             job was terminated at all"
        );
    }

    // Same race, for `pause` -- a controller registering after a Ctrl-Z/`pause` already
    // landed must still be told to pause, not just whichever controller happened to
    // exist at that moment.
    #[test]
    fn a_controller_registered_after_pause_still_gets_paused() {
        let handle = JobHandle::new(JobId::from(1));
        let command_handle = handle.next_command_handle();

        let (early, _, early_pause_calls) = SpyController::new();
        command_handle.register(Arc::from(early));

        let controllers = handle.live_job.lock().unwrap().pause().unwrap();
        notify_pause(&controllers);
        assert_eq!(early_pause_calls.load(Ordering::SeqCst), 1);

        let (late, _, late_pause_calls) = SpyController::new();
        command_handle.register(Arc::from(late));

        assert_eq!(
            late_pause_calls.load(Ordering::SeqCst),
            1,
            "a controller that registers after the job was already paused must be told \
             immediately"
        );
    }

    // Regression test for the deadlock the replay fix above introduced, and this test
    // caught before it ever shipped: `register_command`'s replay call used to run
    // *while still holding* `live_job`'s lock. A `ChannelBasedController` sending into
    // a bounded channel nobody is draining yet blocks -- and since every other
    // command under the same job (e.g. a sibling worker thread calling
    // `next_command_handle` to register its own controller) needs that exact same
    // lock, one controller with a full channel could freeze the entire job, not just
    // itself. Confirmed directly against the real bug: `cmd sleep 100 &` immediately
    // followed by `terminate`/`fg` deadlocked this way under real OS thread
    // scheduling, with `fg`'s own `resume_foreground` call blocked sending into
    // `cmd`'s already-full one-slot control channel while still holding the job's
    // lock, starving `cmd`'s own `next_command_handle` call for a lock that was never
    // coming back.
    #[test]
    fn register_command_does_not_hold_the_job_lock_while_replaying_to_a_blocked_controller() {
        let handle = JobHandle::new(JobId::from(1));
        let blocked_command = handle.next_command_handle();

        handle.live_job.lock().unwrap().terminate().unwrap();

        // A one-slot channel, already full -- `ChannelBasedController::terminate`'s
        // send into it will block forever, since nothing in this test ever drains it.
        let (sender, _receiver_kept_alive_but_never_drained) = crossbeam::channel::bounded(1);
        sender.send(StreamControlMessage::Resume).unwrap();
        let blocking_controller = Arc::from(ChannelBasedController::new(sender));

        std::thread::spawn(move || {
            // Expected to block forever inside the replay's `controller.terminate()`
            // call -- this thread is deliberately never joined.
            blocked_command.register(blocking_controller);
        });

        // Give the spawned thread a moment to actually reach (and block inside) the
        // replay call, then confirm a totally unrelated command on the *same* job can
        // still get the job's lock to register its own controller -- proving the lock
        // isn't held hostage by the other thread's block.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let (done_sender, done_receiver) = crossbeam::channel::bounded(1);
        let handle2 = handle.clone();
        std::thread::spawn(move || {
            let _ = handle2.next_command_handle();
            let _ = done_sender.send(());
        });
        assert!(
            done_receiver
                .recv_timeout(std::time::Duration::from_millis(500))
                .is_ok(),
            "next_command_handle on an unrelated command was still blocked 500ms after \
             a different controller's replay call started blocking on a full channel -- \
             the job's lock is being held hostage across that call"
        );
    }

    // A job that was terminated once and then resumed (e.g. an ineffective terminate
    // against a job that never actually died) must NOT replay a stale terminate to a
    // controller registering afterward -- `termination_requested` stays true forever
    // once set (see its own doc comment), so gating replay on raw `status ==
    // Terminated` alone would wrongly kill a controller for a job that is, right now,
    // actually running again.
    #[test]
    fn a_controller_registered_after_resume_following_an_ineffective_terminate_is_not_terminated() {
        let handle = JobHandle::new(JobId::from(1));
        let command_handle = handle.next_command_handle();

        {
            let mut live_job = handle.live_job.lock().unwrap();
            live_job.terminate().unwrap();
            live_job.resume().unwrap();
        }

        let (late, late_terminate_calls, _) = SpyController::new();
        command_handle.register(Arc::from(late));

        assert_eq!(
            late_terminate_calls.load(Ordering::SeqCst),
            0,
            "a job that was resumed after an ineffective terminate is running again -- a \
             controller registering now must not be told to terminate"
        );
    }
}
