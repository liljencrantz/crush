use crate::lib::ExecutionContext;
use crate::errors::{CrushResult, argument_error, to_job_error};
use crate::{
    data::Row,
    data::ValueType,
    stream::{OutputStream},
    data::Value,
};
use psutil::process::State;
use crate::lib::command_util::{create_user_map, UserMap};
use users::uid_t;
use crate::data::{ColumnType, Command};
use chrono::Duration;
use crate::scope::Scope;
use nix::sys::signal;
use nix::unistd::Pid;
use std::str::FromStr;

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
    let output = context.output.initialize(vec![
        ColumnType::named("pid", ValueType::Integer),
        ColumnType::named("ppid", ValueType::Integer),
        ColumnType::named("status", ValueType::Text),
        ColumnType::named("user", ValueType::Text),
        ColumnType::named("cpu", ValueType::Duration),
        ColumnType::named("name", ValueType::Text),
    ])?;
    let users = create_user_map();

    for proc in &psutil::process::all().unwrap() {
        output.send(Row::new(vec![
            Value::Integer(proc.pid as i128),
            Value::Integer(proc.ppid as i128),
            Value::text(state_name(proc.state)),
            users.get_name(proc.uid as uid_t),
            Value::Duration(Duration::microseconds((proc.utime*1000000.0) as i64)),
            Value::text(
                proc.cmdline_vec().unwrap_or_else(|_| Some(vec!["<Illegal name>".to_string()]))
                    .unwrap_or_else(|| vec![format!("[{}]", proc.comm)])[0]
                    .as_ref()),
        ]))?;
    }
    Ok(())
}

fn kill(context: ExecutionContext) -> CrushResult<()> {
    let mut pids = Vec::new();
    let mut sig_to_send = signal::SIGTERM;

    for arg in context.arguments {
        match (arg.name.as_deref(), arg.value) {
            (None, Value::Integer(pid)) => pids.push(Pid::from_raw(pid as i32)),
            (Some("pid"), Value::Integer(pid)) => pids.push(Pid::from_raw(pid as i32)),
            (Some("signal"), Value::Text(sig)) => sig_to_send = to_job_error(signal::Signal::from_str(sig.as_ref()))?,
            _ => return argument_error("Unknown argument")
        }
    }
    for pid in pids {
        signal::kill(pid, sig_to_send);
    }
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("proc")?;
    root.uses(&env);
    env.declare_str("ps", Value::Command(Command::new(ps)))?;
    env.declare_str("kill", Value::Command(Command::new(kill)))?;
    env.readonly();
    Ok(())
}
