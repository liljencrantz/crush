use crate::lang::errors::CrushResult;
use crate::lang::job_control::JobController;
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

    pub fn register_command(&self, command_handle: &CommandHandle, controller: JobController) {
        let mut live_job = self.live_job.lock().unwrap();
        match live_job.senders.entry(command_handle.id) {
            Entry::Occupied(mut e) => e.get_mut().push(controller),
            Entry::Vacant(e) => {
                e.insert(vec![controller]);
            }
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

    /// True if this job's live status currently reads `Terminated`.
    ///
    /// `GlobalState::terminate` sets this *synchronously*, inside the same call that
    /// broadcasts a `StreamControlMessage::Terminate` to this job's registered
    /// controllers -- independent of whether any of those controllers' own command
    /// threads ever actually get joined, let alone whether a specific join call
    /// happens to observe their real result. That join is *not* race-free:
    /// `ThreadStore::join_job` first collects the ids of every thread currently
    /// registered under this job, then joins each separately -- and in the gap between
    /// those two steps, `ThreadStore::reap` (called by *any* new job being created
    /// anywhere in the system, not just this one) can win the race to remove and join
    /// one of those threads itself first, leaving the original `join_job` call with
    /// nothing left to find for that thread and so no error to propagate (see
    /// `ThreadStore::join_one`: finding nothing for an id is treated the same as "that
    /// thread finished cleanly"). Checking this status flag instead sidesteps that race
    /// entirely: it's set the moment termination is requested, regardless of how or
    /// when (or by whom) this job's thread is eventually reaped. Confirmed directly:
    /// `{sleep $(duration:of seconds=100); echo SHOULD_NOT_APPEAR} &` followed by
    /// `terminate` on the whole block still occasionally ran the `echo` statement
    /// anyway, before `Closure::eval_inner` started checking this (see
    /// `tests/terminate_mid_closure_statement_stops_next_statement.crush`).
    pub fn is_terminated(&self) -> bool {
        let live_job = self.live_job.lock().unwrap();
        live_job.status() == JobStatus::Terminated
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

impl JobControlData {
    pub fn status(&self) -> JobStatus {
        self.status
    }

    pub fn terminate(&mut self) -> CrushResult<()> {
        for vc in self.senders.values() {
            for c in vc {
                let _ = c.terminate();
            }
        }
        self.status = JobStatus::Terminated;
        Ok(())
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

    pub fn pause(&mut self) -> CrushResult<()> {
        for (_, vc) in &self.senders {
            for c in vc {
                let _ = c.pause();
            }
        }
        self.status = JobStatus::Paused;
        Ok(())
    }

    pub fn resume(&mut self) -> CrushResult<()> {
        for vc in self.senders.values() {
            for c in vc {
                let _ = c.resume();
            }
        }
        self.status = JobStatus::Running;
        Ok(())
    }

    /// Like `resume`, but for a job being brought into the foreground (`fg`) rather
    /// than just unpaused in place (`bg`/`resume`) -- see
    /// `JobControl::resume_foreground`.
    pub fn resume_foreground(&mut self) -> CrushResult<()> {
        for vc in self.senders.values() {
            for c in vc {
                let _ = c.resume_foreground();
            }
        }
        self.status = JobStatus::Running;
        Ok(())
    }
}
