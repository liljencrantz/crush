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
                description: String::new(),
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
        live_job.description = name.into();
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

    pub fn description(&self) -> String {
        let live_job = self.live_job.lock().unwrap();
        live_job.description.clone()
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
    pub description: String,
    pub status: JobStatus,
}

pub struct JobControlData {
    pub description: String,
    status: JobStatus,
    senders: HashMap<CommandId, Vec<JobController>>,
    next_command_id: CommandId,
}

pub struct JobData {
    pub id: JobId,
    pub job_type: JobType,
    pub job_control_data: Weak<Mutex<JobControlData>>,
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
}
