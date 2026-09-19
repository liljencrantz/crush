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
use std::collections::HashSet;
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
    /// exited, that's *why* the channel disconnected, so the join is immediate) or if
    /// the value received is a plain, non-stream one *and* `job_id` isn't a background
    /// job. A job's output travels over a channel bounded to exactly one value (`pipe()`
    /// in `crate::lang::pipe`), and a job sends its one top-level value exactly once --
    /// so once a plain value has been received, the thread that sent *that value* is
    /// provably done with every channel it could ever block on; all that's left of it is
    /// bounded, CPU-only teardown (return through its call frames, unregister). Blocking
    /// on `join_job` here cannot deadlock and resolves in microseconds regardless of
    /// system load, and doing so (rather than the non-blocking check used to use)
    /// matters: something checking "is any other job still live" right after this call
    /// (e.g. `crush:exit`'s refusal to exit with jobs running) needs a real answer, not a
    /// best-effort one that can still say "yes" for a thread that has, in every way that
    /// matters, already finished.
    ///
    /// The background-job check is essential, not an edge case: `Job::eval`'s
    /// `is_background` branch (`job &`) sends back a plain job-id value the moment it has
    /// registered the *real* pipeline with `add_background_job` -- but that real
    /// pipeline's own stages keep running, deliberately unjoined, under this exact same
    /// `job_id`, for as long as it takes (a `fs:watch` producer can run forever). Blindly
    /// `join_job`-ing on receipt of that job-id value would defeat the entire point of
    /// `&`, blocking here until the backgrounded work finishes instead of returning
    /// immediately. `job_id`'s presence in `is_background_job` is race-free to check:
    /// `add_background_job` always runs, in the same thread, strictly before the value
    /// announcing it is sent, so by the time it's been received here the registration is
    /// already visible. A *streaming* result is the other case that keeps this
    /// non-blocking, since its producer can legitimately still be doing real, unbounded
    /// work (e.g. `files / --recurse` still walking a large tree) well after handing back
    /// its stream handle -- exactly the case where a caller like `crush:exit` must keep
    /// seeing it as live for as long as that's true, same as a background job.
    ///
    /// Either way, joining also recovers the job's real error instead of the generic
    /// channel-disconnection one `recv()` alone would otherwise surface.
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
                if self.is_background_job(job.id()) {
                    self.threads().try_join_job(job.id())?;
                } else {
                    self.threads().join_job(job.id())?;
                }
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

    /// Terminates `jid`, and every job nested inside it (see `descendant_job_ids`'s doc
    /// comment) -- so terminating a job that's currently running a closure (e.g. one of
    /// `stream:tee`'s branches) also reaches whatever that closure's own body is doing,
    /// not just `jid`'s own directly-registered controllers.
    pub fn terminate(&self, jid: JobId) -> CrushResult<()> {
        let mut data = self.data.lock().unwrap();
        let res = get_job(&mut data, jid, false)?.lock()?.terminate();
        for id in descendant_job_ids(&data, jid) {
            if let Ok(job) = get_job(&mut data, id, false) {
                if let Ok(mut guard) = job.lock() {
                    let _ = guard.terminate();
                }
            }
        }
        res
    }

    /// Like `terminate`, but pauses `jid` and every job nested inside it.
    pub fn pause(&self, jid: JobId) -> CrushResult<()> {
        let mut data = self.data.lock().unwrap();
        let res = get_job(&mut data, jid, true)?.lock()?.pause();
        for id in descendant_job_ids(&data, jid) {
            if let Ok(job) = get_job(&mut data, id, false) {
                if let Ok(mut guard) = job.lock() {
                    let _ = guard.pause();
                }
            }
        }
        res
    }

    /// Like `terminate`, but resumes `jid` and every job nested inside it.
    pub fn resume(&self, jid: JobId) -> CrushResult<()> {
        let mut data = self.data.lock().unwrap();
        let res = get_job(&mut data, jid, true)?.lock()?.resume();
        for id in descendant_job_ids(&data, jid) {
            if let Ok(job) = get_job(&mut data, id, false) {
                if let Ok(mut guard) = job.lock() {
                    let _ = guard.resume();
                }
            }
        }
        res
    }

    /// Register a job started in the background (i.e. `Job::eval()` saw its
    /// `is_background` flag set): `value` is the receiver for whatever the pipeline's
    /// actual last stage eventually sends, to be retrieved later via `fg`.
    pub fn add_background_job(&self, job_id: JobId, value: ValueReceiver) {
        let mut data = self.data.lock().unwrap();
        data.background_jobs.push(BackgroundJob { job_id, value });
    }

    /// True if `job_id` is currently registered as a background job (started with a
    /// trailing `&`, not yet `fg`'d). See `recv_job_result`'s doc comment for why this
    /// matters: a background job's own real work is deliberately left running, unjoined,
    /// under this same `job_id` even after the wrapper value announcing it has already
    /// been delivered -- so, unlike an ordinary plain value, receiving that announcement
    /// must never be treated as "this job_id has nothing left to do".
    pub fn is_background_job(&self, job_id: JobId) -> bool {
        let data = self.data.lock().unwrap();
        data.background_jobs.iter().any(|job| job.job_id == job_id)
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

/// Every currently-live job nested (directly or transitively) inside `jid` -- e.g. a
/// closure/block body being evaluated as part of a job somewhere under `jid`, however
/// many levels deep -- found by walking `parent` links (see `JobData::parent` and
/// `create_nested_job_handle`), not including `jid` itself.
///
/// `Job::eval`'s own `is_background`/foreground join logic, `ThreadStore::join_job`, and
/// everything else that tracks a *job's own threads* already works correctly without
/// this: a closure's body shares the thread-tracking `CommandHandle`/`ThreadStore`
/// machinery of whichever job dispatched it, so joining/waiting still sees those
/// threads. What a nested job gets that's genuinely its *own* is a fresh
/// `JobControlData` (`create_nested_job_handle`), which is what `crush:terminate`/
/// `crush:pause`/`crush:resume` (`GlobalState::terminate`/`pause`/`resume`) actually
/// send a message to -- so without this, telling a job to stop would leave anything
/// still running inside a closure it's currently evaluating completely unreachable,
/// e.g. a `stream:tee` branch's own command never seeing termination even after tee
/// itself does.
///
/// Guards against revisiting an id: `JobId`s are recycled (`next_id` hands out the
/// smallest currently-unused one) the moment a job's own entry is pruned, and a fast
/// enough producer of short-lived nested jobs (e.g. a busy `while` loop dispatching a
/// trivial condition/body closure on every single iteration) can make a *snapshot* of
/// `data.jobs` -- taken once, up front, while this whole function holds the lock, so it
/// can't itself change mid-scan -- contain parent links that trace a cycle purely from
/// id reuse across what were, at different real moments, entirely unrelated jobs (e.g.
/// `0`'s recorded parent is `3`, `3`'s is `1`, `1`'s is `0`). Without a visited set, that
/// cycle sends this into an infinite loop -- confirmed directly: a `while {$true} {}`
/// stream:tee branch reproduced exactly this shape and hung here.
fn descendant_job_ids(data: &MutexGuard<StateData>, jid: JobId) -> Vec<JobId> {
    let mut visited = HashSet::new();
    visited.insert(jid);
    let mut result = Vec::new();
    let mut frontier = vec![jid];
    while let Some(current) = frontier.pop() {
        for jd in &data.jobs {
            if jd.parent == Some(current) && visited.insert(jd.id) {
                result.push(jd.id);
                frontier.push(jd.id);
            }
        }
    }
    result
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
