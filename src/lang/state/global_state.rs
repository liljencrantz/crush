use crate::interactive::rustyline_helper::RustylineHelper;
use crate::lang::ast::lexer::LanguageMode;
use crate::lang::command::Command;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::parser::Parser;
use crate::lang::printer::Printer;
use crate::lang::state::handles::{JobHandle, JobControlData, JobData, JobInfo};
use crate::lang::state::id::JobId;
use crate::lang::threads::ThreadStore;
use crate::util::byte_unit::ByteUnit;
use crate::util::temperature::Temperature;
use num_format::{Grouping, SystemLocale};
use rustyline::Editor;
use rustyline::history::DefaultHistory;
use std::mem;
use std::sync::{Arc, Mutex, MutexGuard};

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
    jobs: Vec<JobData>,
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

    pub fn create_job_handle(&self, fg: bool) -> JobHandle {
        let mut data = self.data.lock().unwrap();
        remove_finished_jobs(&mut data);
        let id = next_id(&data);
        let job = JobHandle::new(id);
        let jd = JobData{id, fg, job_control_data: job.weak_ref()};
        data.jobs.push(jd);
        job
    }

    pub fn current_job(&self) -> Option<JobHandle> {
        let data = self.data.lock().unwrap();
        for jd in data.jobs.iter().rev() {
            if !jd.fg {
                continue
            } 
            match jd.job_control_data.upgrade() {
                Some(arc) => return Some(JobHandle::from(jd.id, arc)),
                None => {}
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

    pub fn jobs(&self) -> Vec<JobInfo> {
        let data = self.data.lock().unwrap();
        let mut res = Vec::new();
        for jd in data.jobs.iter() {
            match jd.job_control_data.upgrade() {
                Some(arc) => {
                    let live_job = arc.lock().unwrap();
                    res.push(JobInfo {
                        id: jd.id,
                        fg: jd.fg,
                        description: live_job.description.clone(),
                        status: live_job.status(),
                    });
                }
                None => {}
            }
        }
        res
    }

    pub fn terminate(&self, jid: JobId) -> CrushResult<()> {
        let mut data = self.data.lock().unwrap();
        get_job(&mut data, jid, false)?.lock()?.terminate()
    }

    pub fn pause(&self, jid: JobId) -> CrushResult<()> {
        let mut data = self.data.lock().unwrap();
        get_job(&mut data, jid, true)?.lock()?.pause()
    }

    pub fn resume(&self, jid: JobId) -> CrushResult<()> {
        let mut data = self.data.lock().unwrap();
        get_job(&mut data, jid, true)?.lock()?.resume()
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

fn next_id(data: &MutexGuard<StateData>) -> JobId {
    for new_id in 0usize.. {
        let mut ok = true;
        for jd in &data.jobs {
            if new_id == usize::from(jd.id) {
                ok = false;
                break;
            }
        }
        if ok {
            return JobId::from(new_id);
        }
    }
    unreachable!()
}

fn get_job(data: &mut MutexGuard<StateData>, target_id: JobId, fg: bool) -> CrushResult<Arc<Mutex<JobControlData>>> {
    for (idx, jd) in data.jobs.iter().enumerate() {
        if jd.id == target_id {
            match jd.job_control_data.upgrade() {
                None => return command_error(format!("Unknown job `{}`", target_id)),
                Some(arc) => {
                    if fg {
                        let tmp = data.jobs.remove(idx);
                        data.jobs.push(tmp);
                    }
                    return Ok(arc)
                },
            }
        }
    }
    command_error(format!("Unknown job `{}`", target_id))
}

fn remove_finished_jobs(data: &mut MutexGuard<StateData>) {
    let mut res = Vec::new();
    for jd in data.jobs.drain(..) {
        match jd.job_control_data.strong_count() {
            0 => {}
            _ => {
                res.push(jd);
            }
        }
    }
    mem::swap(&mut data.jobs, &mut res);
}
