use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::{
    data::Row,
    data::ValueType,
    stream::{OutputStream},
    data::Value,
};
use psutil::process::State;
use crate::commands::command_util::{create_user_map,UserMap};
use users::uid_t;
use crate::data::ColumnType;
use std::time::Duration;

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

pub fn run(output: OutputStream) -> JobResult<()> {
    let users = create_user_map();

    for proc in &psutil::process::all().unwrap() {
        output.send(Row {
            cells: vec![
                Value::Integer(proc.pid as i128),
                Value::Integer(proc.ppid as i128),
                Value::text(state_name(proc.state)),
                users.get_name(proc.uid as uid_t),
                Value::Duration(Duration::from_micros((proc.utime*1000000.0) as u64)),
                Value::text(
                    proc.cmdline_vec().unwrap_or_else(|_| Some(vec!["<Illegal name>".to_string()]))
                        .unwrap_or_else(|| vec![format!("[{}]", proc.comm)])[0]
                        .as_ref()),
            ]
        })?;
    }
    Ok(())
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(vec![
        ColumnType::named("pid", ValueType::Integer),
        ColumnType::named("ppid", ValueType::Integer),
        ColumnType::named("status", ValueType::Text),
        ColumnType::named("user", ValueType::Text),
        ColumnType::named("cpu", ValueType::Duration),
        ColumnType::named("name", ValueType::Text),
    ])?;
    run(output)
}
