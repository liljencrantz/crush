use crate::lang::errors::CrushResult;
use crate::lang::job_control::JobController;
use crate::lang::state::id::CommandId;
use crate::lang::state::id::JobId;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone)]
pub struct JobHandle {
    id: JobId,
    live_job: Arc<Mutex<LiveJob>>,
}

impl JobHandle {
    pub fn new(id: JobId) -> JobHandle {
        JobHandle {
            id,
            live_job: Arc::from(Mutex::from(LiveJob {
                description: String::new(),
                senders: HashMap::new(),
                next_command_id: CommandId::first(),
            })),
        }
    }

    pub fn from(id: JobId, live_job: Arc<Mutex<LiveJob>>) -> JobHandle {
        JobHandle { id, live_job }
    }

    pub fn set_name(&self, name: impl Into<String>) {
        let mut live_job = self.live_job.lock().unwrap();
        live_job.description = name.into();
    }

    pub fn command_handle(&self, id: CommandId) -> CommandHandle {
        CommandHandle {
            job_id: self.clone(),
            id,
        }
    }

    pub fn next_command_handle(&self) -> CommandHandle {
        let mut live_job = self.live_job.lock().unwrap();
        let id = live_job.next_command_id;
        live_job.next_command_id.next();
        CommandHandle {
            job_id: self.clone(),
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

    pub fn weak_ref(&self) -> Weak<Mutex<LiveJob>> {
        Arc::downgrade(&self.live_job)
    }

    pub fn description(&self) -> String {
        let live_job = self.live_job.lock().unwrap();
        live_job.description.clone()
    }
}

#[derive(Clone)]
pub struct CommandHandle {
    pub job_id: JobHandle,
    pub id: CommandId,
}

impl CommandHandle {
    pub fn register(&self, controller: JobController) {
        self.job_id.register_command(self, controller);
    }

    pub fn unregister(&self) {
        self.job_id.unregister_command(self);
    }
}

pub struct LiveJob {
    pub description: String,
    senders: HashMap<CommandId, Vec<JobController>>,
    next_command_id: CommandId,
}

impl LiveJob {
    pub fn terminate(&self) -> CrushResult<()> {
        for vc in self.senders.values() {
            for c in vc {
                let _ = c.terminate();
            }
        }
        Ok(())
    }
}
