use crate::lang::errors::{CrushResult, argument_error, to_crush_error};
use crate::{
    lang::table::Row,
    lang::value::ValueType,
    lang::value::Value,
};
use crate::util::user_map::{create_user_map, UserMap};
use psutil::process::State;
use users::uid_t;
use crate::lang::{table::ColumnType};
use chrono::Duration;
use crate::lang::scope::Scope;
use nix::sys::signal;
use nix::unistd::Pid;
use std::str::FromStr;
use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use lazy_static::lazy_static;
use signature::signature;
use crate::lang::argument::ArgumentHandler;

lazy_static! {
    static ref PS_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("pid", ValueType::Integer),
        ColumnType::new("ppid", ValueType::Integer),
        ColumnType::new("status", ValueType::String),
        ColumnType::new("user", ValueType::String),
        ColumnType::new("cpu", ValueType::Duration),
        ColumnType::new("name", ValueType::String),
    ];
}

fn state_name(s: psutil::process::State) -> &'static str {
    match s {
        State::Running => "Running",
        State::Sleeping => "Sleeping",
        State::Waiting => "Waiting",
        State::Stopped => "Stopped",
        State::Traced => "Traced",
        State::Paging => "Paging",
        State::Dead => "Dead",
        State::Zombie => "Zombie",
        State::Idle => "Idle",
    }
}

fn ps(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let output = context.output.initialize(PS_OUTPUT_TYPE.clone())?;
    let users = create_user_map();

    for proc in &psutil::process::all().unwrap() {
        output.send(Row::new(vec![
            Value::Integer(proc.pid as i128),
            Value::Integer(proc.ppid as i128),
            Value::string(state_name(proc.state)),
            users.get_name(proc.uid as uid_t),
            Value::Duration(Duration::microseconds((proc.utime * 1_000_000.0) as i64)),
            Value::string(
                proc.cmdline_vec().unwrap_or_else(|_| Some(vec!["<Illegal name>".to_string()]))
                    .unwrap_or_else(|| vec![format!("[{}]", proc.comm)])[0]
                    .as_ref()),
        ]))?;
    }
    Ok(())
}

#[signature]
struct KillSignature {
    #[unnamed]
    pid: Vec<i128>,
    #[default("SIGTERM")]
    signal: String,
}

fn kill(context: ExecutionContext) -> CrushResult<()> {
    let sig: KillSignature = KillSignature::parse(context.arguments)?;
    for pid in sig.pid {
        to_crush_error(signal::kill(
            Pid::from_raw(pid as i32),
            to_crush_error(signal::Signal::from_str(&sig.signal))?))?;
    }
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_lazy_namespace(
        "proc",
        Box::new(move |env| {
            env.declare_command(
                "ps", ps, true,
                "ps", "Return a table stream containing information on all running processes on the system.",
                Some(r#"    ps accepts no arguments. Each row contains the following columns:

    * pid:integer the process id of the process

    * ppid:integer the process id of the parent of the process

    * status:string one of the following states:
      - Running
      - Sleeping
      - Waiting
      - Stopped
      - Traces
      - Paging
      - Dead
      - Zombie
      - Idle

    * user:string the username of the process owner

    * cpu:duration the amount of CPU time this process has used since its creation

    * name:string the process name"#))?;

            env.declare_command(
                "kill", kill, false,
                "kill [signal=signal:string] [pid=pid:integer...] @pid:integer",
                "Send a signal to a set of processes",
                Some(r"    Kill accepts the following arguments:

    * signal:string the name of the signal to send. If unspecified, the kill signal is sent.
      The set of existing signals is platform dependent, but common signals include
      SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE, SIGKILL,
      SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE, SIGALRM, SIGTERM, SIGCHLD, SIGCONT and SIGWINCH.

    * pid:integer the process ids of all process to signal."))?;

            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
