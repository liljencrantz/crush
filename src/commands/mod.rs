mod command_util;

mod ls_and_find;

mod echo;

mod pwd;
mod cd;

mod set;
mod let_command;

mod head;
mod tail;

mod lines;
mod csv;

mod filter;
mod sort;
mod select;
mod enumerate;
mod group;
mod join;
mod count;

use crate::stream::{InputStream, OutputStream};
use crate::data::{CellType, Argument, Command};
use crate::state::State;
use crate::errors::{JobError, error};
use std::{io, thread};
use crate::namespace::Namespace;
use std::thread::JoinHandle;
use std::error::Error;
use crate::data::cell::Cell;

type Run = fn(
    Vec<CellType>,
    Vec<Argument>,
    InputStream,
    OutputStream) -> Result<(), JobError>;

type Mutate = fn(
    &mut State,
    Vec<CellType>,
    Vec<Argument>) -> Result<(), JobError>;

pub enum Exec {
    Run(Run),
    Mutate(Mutate),
}

pub enum JobResult {
     Async(JoinHandle<Result<(), JobError>>),
    Sync(Result<(), JobError>),
}

impl JobResult {
    pub fn join(self) -> Result<(), JobError> {
        return match self {
            JobResult::Async(a) => match a.join() {
                Ok(r) => r,
                Err(_) => Err(error("Error while wating for command to finish")),
            },
            JobResult::Sync(s) => s,
        }
    }
}

pub struct Call {
    name: String,
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    output_type: Vec<CellType>,
    exec: Exec,
}

impl Call {
    pub fn get_name(&self) -> &String {
        return &self.name;
    }

    pub fn get_arguments(&self) -> &Vec<Argument> {
        return &self.arguments;
    }

    pub fn get_input_type(&self) -> &Vec<CellType> {
        return &self.input_type;
    }

    pub fn get_output_type(&self) -> &Vec<CellType> {
        return &self.output_type;
    }

    pub fn execute(
        self,
        state: &mut State,
        input: InputStream,
        output: OutputStream) -> JobResult {
        return match self.exec {
            Exec::Run(run) =>
                JobResult::Async(thread::spawn(move || {
                    return run(self.input_type, self.arguments, input, output);
                })),
            Exec::Mutate(mutate) =>
                JobResult::Sync(mutate(state, self.input_type, self.arguments)),
        };
    }
}

fn to_runtime_error<T>(io_result: io::Result<T>) -> Result<T, JobError> {
    match io_result {
        Ok(v) => Ok(v),
        Err(e) => Err(error(e.description())),
    }
}

pub fn add_builtins(namespace: &mut Namespace) -> Result<(), JobError> {
    namespace.declare("ls", Cell::Command(Command::new(ls_and_find::ls)))?;
    namespace.declare("find", Cell::Command(Command::new(ls_and_find::find)))?;
    namespace.declare("echo", Cell::Command(Command::new(echo::echo)))?;
    namespace.declare("pwd", Cell::Command(Command::new(pwd::pwd)))?;
    namespace.declare("cd", Cell::Command(Command::new(cd::cd)))?;
    namespace.declare("filter", Cell::Command(Command::new(filter::filter)))?;
    namespace.declare("sort", Cell::Command(Command::new(sort::sort)))?;
    namespace.declare("set", Cell::Command(Command::new(set::set)))?;
    namespace.declare("let", Cell::Command(Command::new(let_command::let_command)))?;
    namespace.declare("group", Cell::Command(Command::new(group::group)))?;
    namespace.declare("join", Cell::Command(Command::new(join::join)))?;
    namespace.declare("count", Cell::Command(Command::new(count::count)))?;
    namespace.declare("head", Cell::Command(Command::new(head::head)))?;
    namespace.declare("tail", Cell::Command(Command::new(tail::tail)))?;
    namespace.declare("lines", Cell::Command(Command::new(lines::lines)))?;
    namespace.declare("csv", Cell::Command(Command::new(csv::csv)))?;
    namespace.declare("select", Cell::Command(Command::new(select::select)))?;
    namespace.declare("enumerate", Cell::Command(Command::new(enumerate::enumerate)))?;
    return Ok(());
}
