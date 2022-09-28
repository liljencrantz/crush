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
use crate::util::temperature::Temperature;

/**
A type representing the shared crush state, such as the printer, the running jobs, the running
threads, etc.
 */

#[derive(Clone)]
pub struct FormatData {
    locale: SystemLocale,
    temperature: Option<Temperature>,
    float_precision: u8,
    temperature_precision: u8,
    percentage_precision: u8,
}

fn country(locale: & str) -> Option<&str> {
    let dot_split = locale.splitn(2, '.').collect::<Vec<_>>();
    let under_split = dot_split[0].splitn(2, '_').collect::<Vec<_>>();
    if under_split.len() == 2 {
        Some(under_split[1])
    } else {
        None
    }
}

impl FormatData {

    pub fn grouping(&self) -> Grouping {
        self.locale.grouping()
    }

    pub fn locale(&self) -> &SystemLocale {
        &self.locale
    }

    pub fn temperature(&self) -> Temperature {
        self.temperature.unwrap_or_else(||{
            match country(self.locale.name()) {
                // Countries that use Fahrenheit
                Some("US") | Some("BS") | Some("PW") |
                Some("BZ") | Some("KY") | Some("FM") |
                Some("MH") => Temperature::Fahrenheit,
                // All other countries use Celsius
                Some(_) => Temperature::Celsius,
                // You didn't bother setting a locale, YOU GET KELVIN AS PUNISHMENT
                None => Temperature::Kelvin,
            }
        })
    }

    pub fn float_precision(&self) -> usize {
        self.float_precision as usize
    }
    pub fn percentage_precision(&self) -> usize {
        self.percentage_precision as usize
    }
    pub fn temperature_precision(&self) -> usize {
        self.temperature_precision as usize
    }
}

#[derive(Clone)]
pub struct GlobalState {
    data: Arc<Mutex<StateData>>,
    threads: ThreadStore,
    printer: Printer,
    parser: Parser,
    editor: Arc<Mutex<Option<Editor<RustylineHelper>>>>,
}

struct StateData {
    format_data: FormatData,
    prompt: Option<Command>,
    jobs: Vec<Option<LiveJob>>,
    exit_status: Option<i32>,
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
        let mut data = self.state.data.lock().unwrap();
        data.jobs[usize::from(self.id)] = None;
        loop {
            match data.jobs.last() {
                Some(None) => data.jobs.pop(),
                _ => break,
            };
        }
    }
}

impl GlobalState {

    pub fn new(printer: Printer) -> CrushResult<GlobalState> {
        let locale = to_crush_error(SystemLocale::default().or_else(|_| {SystemLocale::from_name("C")}))?;
        Ok(GlobalState {
            data: Arc::from(Mutex::new(StateData {
                format_data: FormatData {
                    locale,
                    temperature: None,
                    float_precision: 4,
                    temperature_precision: 1,
                    percentage_precision: 2,
                },
                exit_status: None,
                prompt: None,
                jobs: Vec::new(),
            })),
            threads: ThreadStore::new(),
            printer,
            parser: Parser::new(),
            editor: Arc::from(Mutex::new(None)),
        })
    }

    pub fn parser(&self) -> &Parser {
        &self.parser
    }

    pub fn threads(&self) -> &ThreadStore {
        &self.threads
    }

    pub fn printer(&self) -> &Printer {
        &self.printer
    }

    pub fn format_data(&self) -> FormatData {
        self.data.lock().unwrap().format_data.clone()
    }

    pub fn job_begin(&self, description: String) -> JobHandle {
        let mut data = self.data.lock().unwrap();
        let id = JobId::from(data.jobs.len());
        data.jobs.push(Some(LiveJob { id, description }));
        JobHandle {
            internal: Arc::new(Mutex::new(JobHandleInternal {
                id,
                state: self.clone(),
            })),
        }
    }

    pub fn set_exit_status(&self, status: i32) {
        let mut data = self.data.lock().unwrap();
        data.exit_status = Some(status);
    }

    pub fn exit_status(&self) -> Option<i32> {
        let data = self.data.lock().unwrap();
        data.exit_status
        }

    pub fn set_locale(&self, new_locale: SystemLocale) {
        let mut data = self.data.lock().unwrap();
        data.format_data.locale = new_locale;
    }

    pub fn set_prompt(&self, prompt: Option<Command>) {
        let mut data = self.data.lock().unwrap();
        data.prompt = prompt;
    }

    pub fn prompt(&self) -> Option<Command> {
        let data = self.data.lock().unwrap();
        data.prompt.as_ref().map(|a| a.clone())
    }

    pub fn jobs(&self) -> Vec<LiveJob> {
        let data = self.data.lock().unwrap();
        data.jobs.iter().flat_map(|a| a.clone()).collect()
    }

    pub fn set_editor(&self, editor: Option<Editor<RustylineHelper>>) {
        let mut data = self.editor.lock().unwrap();
        *data = editor;
    }

    pub fn editor(&self) -> MutexGuard<Option<Editor<RustylineHelper>>> {
        self.editor.lock().unwrap()
    }

}
