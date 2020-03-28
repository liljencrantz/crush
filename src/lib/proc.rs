use crate::lang::command::CrushCommand;
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
use crate::lang::execution_context::ExecutionContext;

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
        ColumnType::new("pid", ValueType::Integer),
        ColumnType::new("ppid", ValueType::Integer),
        ColumnType::new("status", ValueType::String),
        ColumnType::new("user", ValueType::String),
        ColumnType::new("cpu", ValueType::Duration),
        ColumnType::new("name", ValueType::String),
    ])?;
    let users = create_user_map();

    for proc in &psutil::process::all().unwrap() {
        output.send(Row::new(vec![
            Value::Integer(proc.pid as i128),
            Value::Integer(proc.ppid as i128),
            Value::string(state_name(proc.state)),
            users.get_name(proc.uid as uid_t),
            Value::Duration(Duration::microseconds((proc.utime*1000000.0) as i64)),
            Value::string(
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
        match (arg.argument_type.as_deref(), arg.value) {
            (None, Value::Integer(pid)) => pids.push(Pid::from_raw(pid as i32)),
            (Some("pid"), Value::Integer(pid)) => pids.push(Pid::from_raw(pid as i32)),
            (Some("signal"), Value::String(sig)) => sig_to_send = to_crush_error(signal::Signal::from_str(sig.as_ref()))?,
            _ => return argument_error("Unknown argument")
        }
    }
    for pid in pids {
        to_crush_error(signal::kill(pid, sig_to_send))?;
    }
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("proc")?;
    root.r#use(&env);
    env.declare("ps", Value::Command(CrushCommand::command(ps, true)))?;
    env.declare("kill", Value::Command(CrushCommand::command(kill, false)))?;
    env.readonly();
    Ok(())
}
