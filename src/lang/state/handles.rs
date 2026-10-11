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
