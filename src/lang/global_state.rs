use num_format::{SystemLocale, Grouping};
use crate::lang::errors::{CrushResult, to_crush_error};
use std::sync::{Arc, Mutex};
use crate::lang::threads::ThreadStore;
use crate::lang::printer::Printer;
use crate::lang::command::Command;

struct StateData {
    locale: SystemLocale,
}

#[derive(Clone)]
pub struct GlobalState {
    data: Arc<Mutex<StateData>>,
    threads: ThreadStore,
    printer: Printer,
    exit_status: Arc<Mutex<Option<i32>>>,
    prompt: Arc<Mutex<Option<Command>>>,
}

impl GlobalState {
    pub fn new(printer: Printer) -> CrushResult<GlobalState> {
        Ok(GlobalState {
            data: Arc::from(Mutex::new(
                StateData {
                    locale: to_crush_error(SystemLocale::default())?,
                }
            )),
            threads: ThreadStore::new(),
            printer,
            exit_status: Arc::from(Mutex::new(None)),
            prompt: Arc::from(Mutex::new(None)),
        })
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

    pub fn set_exit_status(&self, status: i32) {
        let mut data = self.exit_status.lock().unwrap();
        *data = Some(status);
    }

    pub fn exit_status(&self) -> Option<i32> {
        let mut data = self.exit_status.lock().unwrap();
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
}