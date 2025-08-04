use crate::interactive::rustyline_helper::RustylineHelper;
use crate::lang::ast::lexer::LanguageMode;
use crate::lang::command::Command;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::parser::Parser;
use crate::lang::printer::Printer;
use crate::lang::state::handles::{JobHandle, LiveJob};
use crate::lang::state::id::JobId;
use crate::lang::threads::ThreadStore;
use crate::util::byte_unit::ByteUnit;
use crate::util::temperature::Temperature;
use num_format::{Grouping, SystemLocale};
use rustyline::Editor;
use rustyline::history::DefaultHistory;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

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
    byte_unit: ByteUnit,
}

#[derive(Clone, Copy)]
pub enum RunMode {
    Interactive,
    NonInteractive,
}

fn country(locale: &str) -> Option<&str> {
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

    pub fn byte_unit(&self) -> ByteUnit {
        self.byte_unit
    }

    pub fn temperature(&self) -> Temperature {
        self.temperature.unwrap_or_else(|| {
            match country(self.locale.name()) {
                // Countries that use Fahrenheit
                Some("US") | Some("BS") | Some("PW") | Some("BZ") | Some("KY") | Some("FM")
                | Some("MH") => Temperature::Fahrenheit,
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
    editor: Arc<Mutex<Option<Editor<RustylineHelper, DefaultHistory>>>>,
}

struct StateData {
    format_data: FormatData,
    prompt: Option<Command>,
    title: Option<Command>,
    jobs: Vec<Option<Weak<Mutex<LiveJob>>>>,
    exit_status: Option<i32>,
    language_mode: LanguageMode,
    run_mode: RunMode,
}

impl GlobalState {
    pub fn new(printer: Printer, run_mode: RunMode) -> CrushResult<GlobalState> {
        let locale = SystemLocale::default().or_else(|_| SystemLocale::from_name("C"))?;
        Ok(GlobalState {
            data: Arc::from(Mutex::new(StateData {
                format_data: FormatData {
                    locale,
                    temperature: None,
                    float_precision: 4,
                    temperature_precision: 1,
                    percentage_precision: 2,
                    byte_unit: ByteUnit::Binary,
                },
                exit_status: None,
                prompt: None,
                title: None,
                jobs: Vec::new(),
                language_mode: LanguageMode::Command,
                run_mode,
            })),
            threads: ThreadStore::new(),
            printer,
            parser: Parser::new(),
            editor: Arc::from(Mutex::new(None)),
        })
    }

    pub fn create_job_handle(&self) -> JobHandle {
        let mut data = self.data.lock().unwrap();
        while !data.jobs.is_empty() {
            match data.jobs.last() {
                Some(Some(job)) => match job.strong_count() {
                    0 => {}
                    _ => {
                        break;
                    }
                },
                _ => {}
            }
            data.jobs.pop();
        }
        let id = data.jobs.len().into();
        let job = JobHandle::new(id);
        data.jobs.push(Some(job.weak_ref()));
        job
    }

    pub fn current_job(&self) -> Option<JobHandle> {
        let data = self.data.lock().unwrap();
        for (idx, i) in data.jobs.iter().enumerate().rev() {
            if let Some(j) = i {
                match j.upgrade() {
                    Some(arc) => return Some(JobHandle::from(idx.into(), arc)),
                    None => {}
                }
            }
        }
        None
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

    pub fn set_exit_status(&self, status: i32) {
        let mut data = self.data.lock().unwrap();
        data.exit_status = Some(status);
    }

    pub fn exit_status(&self) -> Option<i32> {
        let data = self.data.lock().unwrap();
        data.exit_status
    }

    pub fn set_language_mode(&self, mode: LanguageMode) {
        let mut data = self.data.lock().unwrap();
        data.language_mode = mode;
    }

    pub fn language_mode(&self) -> LanguageMode {
        let data = self.data.lock().unwrap();
        data.language_mode
    }

    pub fn run_mode(&self) -> RunMode {
        let data = self.data.lock().unwrap();
        data.run_mode
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

    pub fn title(&self) -> Option<Command> {
        let data = self.data.lock().unwrap();
        data.title.as_ref().map(|a| a.clone())
    }

    pub fn set_title(&self, prompt: Option<Command>) {
        let mut data = self.data.lock().unwrap();
        data.title = prompt;
    }

    pub fn jobs(&self) -> Vec<(JobId, String)> {
        let data = self.data.lock().unwrap();
        let mut res = Vec::new();
        for (idx, i) in data.jobs.iter().enumerate() {
            if let Some(j) = i {
                match j.upgrade() {
                    Some(arc) => {
                        let live_job = arc.lock().unwrap();
                        res.push((idx.into(), live_job.description.clone()));
                    }
                    None => {}
                }
            }
        }
        res
    }

    pub fn terminate(&self, jid: JobId) -> CrushResult<()> {
        let data = self.data.lock().unwrap();
        match data.jobs.get(usize::from(jid)) {
            Some(Some(weak)) => match weak.upgrade() {
                Some(arc) => arc.lock().unwrap().terminate(),
                None => command_error(format!("Unknown job `{}`", jid)),
            },
            _ => command_error(format!("Unknown job `{}`", jid)),
        }
    }

    pub fn pause(&self, jid: JobId) -> CrushResult<()> {
        let data = self.data.lock().unwrap();
        match data.jobs.get(usize::from(jid)) {
            Some(Some(weak)) => match weak.upgrade() {
                Some(arc) => arc.lock().unwrap().pause(),
                None => command_error(format!("Unknown job `{}`", jid)),
            },
            _ => command_error(format!("Unknown job `{}`", jid)),
        }
    }

    pub fn resume(&self, jid: JobId) -> CrushResult<()> {
        let data = self.data.lock().unwrap();
        match data.jobs.get(usize::from(jid)) {
            Some(Some(weak)) => match weak.upgrade() {
                Some(arc) => arc.lock().unwrap().resume(),
                None => command_error(format!("Unknown job `{}`", jid)),
            },
            _ => command_error(format!("Unknown job `{}`", jid)),
        }
    }

    pub fn set_editor(&self, editor: Option<Editor<RustylineHelper, DefaultHistory>>) {
        let mut data = self.editor.lock().unwrap();
        *data = editor;
    }

    pub fn editor(&self) -> MutexGuard<Option<Editor<RustylineHelper, DefaultHistory>>> {
        self.editor.lock().unwrap()
    }

    pub fn set_byte_unit(&self, b: ByteUnit) {
        self.data.lock().unwrap().format_data.byte_unit = b;
    }

    pub fn set_float_precision(&self, p: u8) {
        self.data.lock().unwrap().format_data.float_precision = p;
    }

    pub fn set_percentage_precision(&self, p: u8) {
        self.data.lock().unwrap().format_data.percentage_precision = p;
    }

    pub fn set_temperature_precision(&self, p: u8) {
        self.data.lock().unwrap().format_data.temperature_precision = p;
    }
}
