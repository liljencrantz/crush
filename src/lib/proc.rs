use crate::lang::argument::ArgumentHandler;
use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::errors::{error, to_crush_error, CrushResult};
use crate::lang::execution_context::{ArgumentVector, CommandContext};
use crate::lang::scope::Scope;
use crate::lang::stream::OutputStream;
use crate::lang::table::ColumnType;
use crate::util::user_map::{create_user_map, UserMap};
use crate::{lang::table::Row, lang::value::Value, lang::value::ValueType};
use chrono::Duration;
use lazy_static::lazy_static;
use nix::sys::signal;
use nix::unistd::Pid;
use psutil::process::os::unix::ProcessExt;
use psutil::process::{Process, ProcessResult, Status};
use signature::signature;
use std::collections::HashMap;
use std::str::FromStr;
use users::{uid_t, User};

lazy_static! {
    static ref PS_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("pid", ValueType::Integer),
        ColumnType::new("ppid", ValueType::Integer),
        ColumnType::new("status", ValueType::String),
        ColumnType::new("user", ValueType::String),
        ColumnType::new("cpu", ValueType::Duration),
        ColumnType::new("rss", ValueType::Integer),
        ColumnType::new("vms", ValueType::Integer),
        ColumnType::new("name", ValueType::String),
    ];
}

#[signature(
ps,
can_block = true,
short = "Return a table stream containing information on all running processes on the system",
output = Known(ValueType::TableStream(PS_OUTPUT_TYPE.clone())),
long = "ps accepts no arguments.")]
struct Ps {
}

fn state_name(s: Status) -> &'static str {
    match s {
        Status::Running => "Running",
        Status::Sleeping => "Sleeping",
        Status::Waiting => "Waiting",
        Status::Stopped => "Stopped",
        Status::Dead => "Dead",
        Status::Zombie => "Zombie",
        Status::Idle => "Idle",
        Status::DiskSleep => "DiskSleep",
        Status::TracingStop => "TracingStop",
        Status::WakeKill => "WakeKill",
        Status::Waking => "Waking",
        Status::Parked => "Parked",
        Status::Locked => "Locked",
        Status::Suspended => "Suspended",
    }
}

fn ps(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let output = context.output.initialize(PS_OUTPUT_TYPE.clone())?;
    let users = create_user_map();

    match psutil::process::processes() {
        Ok(procs) => {
            for proc in procs {
                output.send(to_crush_error(ps_internal(proc, &users))?)?;
            }
        }
        Err(e) => return error("Failed to list processes"),
    }
    Ok(())
}

fn ps_internal(proc: ProcessResult<Process>, users: &HashMap<u32, User>) -> ProcessResult<Row> {
    let mut proc = proc?;
    Ok(Row::new(vec![
        Value::Integer(proc.pid() as i128),
        Value::Integer(proc.ppid()?.unwrap_or(0) as i128),
        Value::string(state_name(proc.status()?)),
        users.get_name(proc.uids()?.effective as uid_t),
        Value::Duration(Duration::microseconds(
            proc.cpu_times()?.busy().as_micros() as i64
        )),
        Value::Integer(proc.memory_info()?.rss() as i128),
        Value::Integer(proc.memory_info()?.vms() as i128),
        Value::string(
            proc.cmdline_vec()?
                .unwrap_or(vec![format!("[{}]", proc.name()?)])[0]
                .as_ref(),
        ),
    ]))
}

#[signature(
kill,
can_block = false,
short = "Send a signal to a set of processes",
output = Known(ValueType::Empty),
long = "The set of existing signals is platform dependent, but common signals
    include SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE,
    SIGKILL, SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE, SIGALRM, SIGTERM, SIGCHLD,
    SIGCONT and SIGWINCH.")]
struct Kill {
    #[unnamed("id of a process to signal")]
    #[description("the name of the signal to send.")]
    pid: Vec<i128>,
    #[default("SIGTERM")]
    #[description("the name of the signal to send.")]
    signal: String,
}

fn kill(context: CommandContext) -> CrushResult<()> {
    let sig: Kill = Kill::parse(context.arguments, &context.printer)?;
    for pid in sig.pid {
        to_crush_error(signal::kill(
            Pid::from_raw(pid as i32),
            to_crush_error(signal::Signal::from_str(&sig.signal))?,
        ))?;
    }
    context.output.send(Value::Empty())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "proc",
        Box::new(move |env| {
            Ps::declare(env)?;
            Kill::declare(env)?;
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
