use crate::{
    data::Argument,
    data::Row,
    data::CellType,
    stream::{OutputStream, InputStream},
    data::Cell,
    commands::Exec,
    errors::JobError,
    env::get_cwd,
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::CellFnurp;
use psutil::process::State;
use crate::commands::command_util::create_user_map;
use users::uid_t;

pub struct Config { output: OutputStream }


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

pub fn run(
    config: Config,
    _env: Env,
    _printer: Printer,
) -> Result<(), JobError> {
    let users = create_user_map();

    for proc in &psutil::process::all().unwrap() {
        config.output.send(Row {
            cells: vec![
                Cell::Integer(proc.pid as i128),
                Cell::Integer(proc.ppid as i128),
                Cell::text(state_name(proc.state)),
                Cell::text(users.get(&(proc.uid as uid_t)).map(|u| u.name().to_str().unwrap_or("<illegal username>")).unwrap_or("<unknown user>")),
                Cell::text(
                    proc.cmdline_vec().unwrap_or_else(|e| Some(vec!["<Illegal name>".to_string()]))
                        .unwrap_or_else(|| vec![format!("[{}]", proc.comm)])[0]
                        .as_ref()),
            ]
        })?;
    }

    Ok(())
}

pub fn compile(_input_type: Vec<CellFnurp>, _input: InputStream, output: OutputStream, _arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    return Ok((Exec::Ps(Config { output }), vec![
        CellFnurp::named("pid", CellType::Integer),
        CellFnurp::named("ppid", CellType::Integer),
        CellFnurp::named("status", CellType::Text),
        CellFnurp::named("owner", CellType::Integer),
        CellFnurp::named("name", CellType::Text),
    ]));
}
