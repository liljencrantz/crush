use num_format::{SystemLocale, Grouping};
use crate::lang::errors::{CrushResult, to_crush_error};
use std::sync::{Arc, Mutex};
use crate::lang::threads::ThreadStore;
use crate::lang::printer::Printer;

struct StateData {
    locale: SystemLocale,
}

#[derive(Clone)]
pub struct GlobalState {
    data: Arc<Mutex<StateData>>,
    threads: ThreadStore,
    printer: Printer,
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

    pub fn set_locale(&self, new_locale: SystemLocale) {
        let mut data = self.data.lock().unwrap();
        data.locale = new_locale;
    }
}