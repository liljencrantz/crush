use crate::interactive::rustyline_helper::RustylineHelper;
use crate::lang::ast::lexer::LanguageMode;
use crate::lang::command::Command;
use crate::lang::errors::{CrushError, CrushResult, command_error};
use crate::lang::parser::Parser;
use crate::lang::pipe::ValueReceiver;
use crate::lang::printer::Printer;
use crate::lang::state::handles::JobType::Background;
use crate::lang::state::handles::{JobControlData, JobData, JobHandle, JobInfo, JobType};
use crate::lang::state::id::JobId;
use crate::lang::state::warning::Warning;
use crate::lang::threads::ThreadStore;
use crate::lang::value::Value;
use crate::util::byte_unit::ByteUnit;
use crate::util::temperature::Temperature;
use num_format::{Grouping, SystemLocale};
use rustyline::Editor;
use rustyline::history::DefaultHistory;
use std::collections::VecDeque;
use std::mem;
use std::sync::{Arc, Mutex, MutexGuard};

/// The default value for how many warnings GlobalState keeps around for later
/// inspection (e.g. via `crush:warn:list`) before evicting the oldest -- adjustable at
/// runtime via `crush:warn:limit:set`.
const DEFAULT_WARNING_LIMIT: usize = 100;

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

/// A job started in the background: `Job::eval()` registers one of these instead of
/// waiting on the pipeline's actual last stage, so `fg` can retrieve its eventual
/// result later.
struct BackgroundJob {
    job_id: JobId,
    value: ValueReceiver,
}

struct StateData {
    format_data: FormatData,
    prompt: Option<Command>,
    title: Option<Command>,
    jobs: Vec<JobData>,
    exit_status: Option<i32>,
    language_mode: LanguageMode,
    run_mode: RunMode,
    warnings: VecDeque<Warning>,
    warning_limit: usize,
    warn_print: bool,
    background_jobs: Vec<BackgroundJob>,
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
                warnings: VecDeque::new(),
                warning_limit: DEFAULT_WARNING_LIMIT,
                warn_print: true,
                background_jobs: Vec::new(),
            })),
            threads: ThreadStore::new(),
            printer,
            parser: Parser::new(),
            editor: Arc::from(Mutex::new(None)),
        })
    }

    pub fn create_job_handle(&self, job_type: JobType) -> JobHandle {
        self.create_job_handle_with_parent(job_type, None)
    }

    /// Like `create_job_handle`, but records `parent` as the job this new job is nested
    /// inside (e.g. a closure/block body being evaluated as part of `parent`). Used so
    /// job-related checks can recognize an ancestor job as not being a genuinely separate,
    /// unrelated one.
    pub fn create_nested_job_handle(&self, job_type: JobType, parent: JobId) -> JobHandle {
        self.create_job_handle_with_parent(job_type, Some(parent))
    }

    fn create_job_handle_with_parent(&self, job_type: JobType, parent: Option<JobId>) -> JobHandle {
        let mut data = self.data.lock().unwrap();
        remove_finished_jobs(&mut data);
        let id = next_id(&data);
        let job = JobHandle::new(id);
        let jd = JobData {
            id,
            job_type,
            parent,
            job_control_data: job.weak_ref(),
        };
        data.jobs.push(jd);
        job
    }

    pub fn current_job(&self) -> Option<JobHandle> {
        let data = self.data.lock().unwrap();
        for jd in data.jobs.iter().rev() {
            if jd.job_type == Background {
                continue;
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

    /// Waits for a job's captured output value without first waiting for every thread
    /// it spawned (a 5-stage pipeline can have up to 5, all sharing `job_id` -- see
    /// `ThreadStore::join_job`) to fully exit -- the job's own last stage still needs to
    /// actually finish producing that value, but nothing here needs the whole job to
    /// have finished *everything* by then. A job that streams its result (e.g. any
    /// table-producing command) keeps sending rows into that stream's own bounded row
    /// channel (see `streams()` in `crate::lang::pipe`) after handing back the stream
    /// handle itself; joining first -- waiting for every stage to fully exit -- before
    /// ever reading that handle would mean nobody drains that channel until the join
    /// returns, so once the row count exceeds the channel's capacity the producer blocks
    /// forever waiting for a reader that can only ever be unblocked by the join it's
    /// blocking. This mirrors how an explicitly backgrounded job (`job &`) already
    /// behaves: `fg` on it also only `recv()`s the job's output, never joins its thread.
    ///
    /// A stream can still fail *after* successfully handing back its handle -- e.g.
    /// `uniq` sends its output stream immediately, then only discovers a non-hashable
    /// value once it actually reads a row -- so a returned `Value::TableInputStream` is
    /// tagged with `job_id`; `TableInputStream::recv()` joins it once the stream is
    /// actually drained to the end, surfacing that failure as a real trailing error
    /// instead of silently treating early termination as clean end-of-stream.
    ///
    /// Every thread under `job_id` is joined immediately, without deferring to that
    /// later drain, if `recv()` itself fails (meaning the job's last stage exited
    /// without ever producing a value at all -- by then every one of them has already
    /// exited, that's *why* the channel disconnected, so the join is immediate) or if an
    /// opportunistic non-blocking check right after a successful `recv()` finds one of
    /// them *already* failed for real (the common case for a small/quick job, and not
    /// something a later stream drain would ever get the chance to catch if this
    /// succeeded value isn't a stream at all). Either way this recovers the job's real
    /// error instead of the generic channel-disconnection one `recv()` alone would
    /// otherwise surface.
    pub fn recv_job_result(&self, job: &JobHandle, last_input: &ValueReceiver) -> CrushResult<Value> {
        match last_input.recv() {
            Ok(Value::TableInputStream(stream)) => {
                self.threads().try_join_job(job.id())?;
                Ok(Value::TableInputStream(
                    stream.with_producer_job(job.clone(), self.threads().clone()),
                ))
            }
            Ok(Value::BinaryInputStream(mut reader)) => {
                self.threads().try_join_job(job.id())?;
                reader.set_producer_job(job.clone(), self.threads().clone());
                Ok(Value::BinaryInputStream(reader))
            }
            Ok(v) => {
                self.threads().try_join_job(job.id())?;
                Ok(v)
            }
            Err(recv_err) => {
                self.threads().join_job(job.id())?;
                Err(recv_err)
            }
        }
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

    /// Report a non-fatal, partial failure. Stores it in the bounded warning log
    /// (evicting the oldest entry past the current warning_limit) and, in interactive
    /// mode, also prints it immediately via the printer -- unless printing has been
    /// disabled via `set_warn_print`.
    pub fn warn(&self, err: &CrushError) {
        let warning = Warning::from_error(err);
        let (run_mode, warn_print) = {
            let mut data = self.data.lock().unwrap();
            data.warnings.push_back(warning.clone());
            while data.warnings.len() > data.warning_limit {
                data.warnings.pop_front();
            }
            (data.run_mode, data.warn_print)
        };
        if let RunMode::Interactive = run_mode {
            if warn_print {
                self.printer.warning(warning);
            }
        }
    }

    /// The current contents of the bounded warning log, oldest first.
    pub fn warnings(&self) -> Vec<Warning> {
        let data = self.data.lock().unwrap();
        data.warnings.iter().cloned().collect()
    }

    /// How many warnings are kept before the oldest gets evicted.
    pub fn warning_limit(&self) -> usize {
        let data = self.data.lock().unwrap();
        data.warning_limit
    }

    /// Whether a warning is also printed immediately (in interactive mode) as soon as
    /// it's reported, rather than only being visible later via `crush:warn:list`.
    pub fn warn_print(&self) -> bool {
        let data = self.data.lock().unwrap();
        data.warn_print
    }

    /// Change whether a warning is also printed immediately (in interactive mode) as
    /// soon as it's reported.
    pub fn set_warn_print(&self, value: bool) {
        let mut data = self.data.lock().unwrap();
        data.warn_print = value;
    }

    /// Change how many warnings are kept before the oldest gets evicted. If the log
    /// already holds more than the new limit, it's trimmed immediately rather than
    /// waiting for the next warning to catch up.
    pub fn set_warning_limit(&self, limit: usize) {
        let mut data = self.data.lock().unwrap();
        data.warning_limit = limit;
        while data.warnings.len() > data.warning_limit {
            data.warnings.pop_front();
        }
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
                        job_type: jd.job_type,
                        description: live_job.description.clone(),
                        status: live_job.status(),
                        parent: jd.parent,
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

    /// Register a job started in the background (i.e. `Job::eval()` saw its
    /// `is_background` flag set): `value` is the receiver for whatever the pipeline's
    /// actual last stage eventually sends, to be retrieved later via `fg`.
    pub fn add_background_job(&self, job_id: JobId, value: ValueReceiver) {
        let mut data = self.data.lock().unwrap();
        data.background_jobs.push(BackgroundJob { job_id, value });
    }

    /// Remove and return the named background job's receiver, if one is registered.
    pub fn take_background_job(&self, job_id: JobId) -> Option<ValueReceiver> {
        let mut data = self.data.lock().unwrap();
        let mut matching = data
            .background_jobs
            .extract_if(.., |job| job.job_id == job_id)
            .collect::<Vec<_>>();
        matching.pop().map(|job| job.value)
    }

    /// Remove and return the most recently backgrounded job's receiver, if any is
    /// registered.
    pub fn take_last_background_job(&self) -> Option<ValueReceiver> {
        let mut data = self.data.lock().unwrap();
        data.background_jobs.pop().map(|job| job.value)
    }

    pub fn set_editor(&self, editor: Option<Editor<RustylineHelper, DefaultHistory>>) {
        let mut data = self.editor.lock().unwrap();
        *data = editor;
    }

    pub fn editor(&self) -> MutexGuard<'_, Option<Editor<RustylineHelper, DefaultHistory>>> {
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

fn get_job(
    data: &mut MutexGuard<StateData>,
    target_id: JobId,
    fg: bool,
) -> CrushResult<Arc<Mutex<JobControlData>>> {
    for (idx, jd) in data.jobs.iter().enumerate() {
        if jd.id == target_id {
            match jd.job_control_data.upgrade() {
                None => return command_error(format!("Unknown job `{}`", target_id)),
                Some(arc) => {
                    if fg {
                        let tmp = data.jobs.remove(idx);
                        data.jobs.push(tmp);
                    }
                    return Ok(arc);
                }
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
