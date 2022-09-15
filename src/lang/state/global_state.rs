use crate::lang::command::Command;
use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::parser::Parser;
use crate::lang::printer::Printer;
use crate::lang::threads::ThreadStore;
use num_format::{Grouping, SystemLocale};
use std::sync::{Arc, Mutex, MutexGuard};
use rustyline::Editor;
use crate::interactive::rustyline_helper::RustylineHelper;
use crate::lang::value::Value;

/**
A type representing the shared crush state, such as the printer, the running jobs, the running
threads, etc.
 */

#[derive(Clone)]
pub struct GlobalState {
    data: Arc<Mutex<StateData>>,
    threads: ThreadStore,
    printer: Printer,
    exit_status: Arc<Mutex<Option<i32>>>,
    prompt: Arc<Mutex<Option<Command>>>,
    jobs: Arc<Mutex<Vec<Option<LiveJob>>>>,
    parser: Parser,
    editor: Arc<Mutex<Option<Editor<RustylineHelper>>>>,
}

struct StateData {
    locale: SystemLocale,
}

#[derive(Clone, Copy)]
pub struct JobId(usize);

impl From<usize> for JobId {
    fn from(id: usize) -> Self {
        JobId(id)
    }
}

impl From<JobId> for usize {
    fn from(id: JobId) -> Self {
        id.0
    }
}

impl From<JobId> for Value {
    fn from(id: JobId) -> Self {
        Value::Integer(id.0 as i128)
    }
}

pub struct JobHandleInternal {
    id: JobId,
    state: GlobalState,
}

#[derive(Clone)]
pub struct LiveJob {
    pub id: JobId,
    pub description: String,
}

/**
  A resource tracker. Once it reaches zero, the job is done, and it is removed from the global job
  table.
*/
#[derive(Clone)]
pub struct JobHandle {
    internal: Arc<Mutex<JobHandleInternal>>,
}

impl JobHandle {
    pub fn id(&self) -> JobId {
        self.internal.lock().unwrap().id
    }
}

impl Drop for JobHandleInternal {
    fn drop(&mut self) {
        let mut data = self.state.jobs.lock().unwrap();
        data[usize::from(self.id)] = None;
        loop {
            match data.last() {
                Some(None) => data.pop(),
                _ => break,
            };
        }
    }
}

impl GlobalState {
    pub fn new(printer: Printer) -> CrushResult<GlobalState> {
        Ok(GlobalState {
            data: Arc::from(Mutex::new(StateData {
                locale: to_crush_error(SystemLocale::default().or_else(|e| {SystemLocale::from_name("C")}))?,
            })),
            threads: ThreadStore::new(),
            printer,
            exit_status: Arc::from(Mutex::new(None)),
            prompt: Arc::from(Mutex::new(None)),
            parser: Parser::new(),
            jobs: Arc::from(Mutex::new(Vec::new())),
            editor: Arc::from(Mutex::new(None)),
        })
    }

    pub fn parser(&self) -> &Parser {
        &self.parser
    }

    pub fn grouping(&self) -> Grouping {
        let data = self.data.lock().unwrap();
        data.locale.grouping()
    }

    pub fn threads(&self) -> &ThreadStore {
        &self.threads
    }

    pub fn printer(&self) -> &Printer {
        &self.printer
    }

    pub fn locale(&self) -> SystemLocale {
        let data = self.data.lock().unwrap();
        data.locale.clone()
    }

    pub fn job_begin(&self, description: String) -> JobHandle {
        let mut jobs = self.jobs.lock().unwrap();
        let id = JobId::from(jobs.len());
        jobs.push(Some(LiveJob { id, description }));
        JobHandle {
            internal: Arc::new(Mutex::new(JobHandleInternal {
                id,
                state: self.clone(),
            })),
        }
    }

    pub fn set_exit_status(&self, status: i32) {
        let mut data = self.exit_status.lock().unwrap();
        *data = Some(status);
    }

    pub fn exit_status(&self) -> Option<i32> {
        let data = self.exit_status.lock().unwrap();
        (*data).clone()
    }

    pub fn set_locale(&self, new_locale: SystemLocale) {
        let mut data = self.data.lock().unwrap();
        data.locale = new_locale;
    }

    pub fn set_prompt(&self, prompt: Option<Command>) {
        let mut data = self.prompt.lock().unwrap();
        *data = prompt;
    }

    pub fn prompt(&self) -> Option<Command> {
        let data = self.prompt.lock().unwrap();
        data.as_ref().map(|a| a.copy())
    }

    pub fn jobs(&self) -> Vec<LiveJob> {
        let jobs = self.jobs.lock().unwrap();
        jobs.iter().flat_map(|a| a.clone()).collect()
    }

    pub fn set_editor(&self, editor: Option<Editor<RustylineHelper>>) {
        let mut data = self.editor.lock().unwrap();
        *data = editor;
    }

    pub fn editor(&self) -> MutexGuard<Option<Editor<RustylineHelper>>> {
        self.editor.lock().unwrap()
    }

}
