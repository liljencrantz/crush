mod command_util;

mod ls_and_find;

mod echo;

mod pwd;
mod cd;

mod set;
mod let_command;
mod unset;

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
mod cat;

mod cast;

use std::{io, thread};
use crate::{
    namespace::Namespace,
    errors::{JobError, error},
    state::State,
    data::{
        CellType,
        Argument,
        Command,
        Cell
    },
    stream::{InputStream, OutputStream},
};
use std::thread::JoinHandle;
use std::error::Error;
use crate::printer::Printer;

type Run = fn(
    Vec<CellType>,
    Vec<Argument>,
    InputStream,
    OutputStream,
    Printer) -> Result<(), JobError>;

type Mutate = fn(
    &mut State,
    Vec<CellType>,
    Vec<Argument>) -> Result<(), JobError>;

#[derive(Clone)]
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
        let printer = state.printer.clone();
        return match self.exec {
            Exec::Run(run) =>
                JobResult::Async(thread::Builder::new().name(self.name.clone()).spawn(move || {
                    return run(self.input_type, self.arguments, input, output, printer);
                }).unwrap()),
            Exec::Mutate(mutate) =>
                JobResult::Sync(mutate(state, self.input_type, self.arguments)),
        };
    }
}

impl Clone for Call {
    fn clone(&self) -> Self {
        Call {
            name: self.name.clone(),
            input_type: self.input_type.clone(),
            arguments: self.arguments.iter()
                .map(|a| {Argument {
            name: a.name.clone(),
            cell: a.cell.partial_clone().unwrap(),
            }}).collect::<Vec<Argument>>(),
            output_type: self.output_type.clone(),
            exec: self.exec.clone(),
        }
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
    namespace.declare("unset", Cell::Command(Command::new(unset::unset)))?;
    namespace.declare("group", Cell::Command(Command::new(group::group)))?;
    namespace.declare("join", Cell::Command(Command::new(join::join)))?;
    namespace.declare("count", Cell::Command(Command::new(count::count)))?;
    namespace.declare("cat", Cell::Command(Command::new(cat::cat)))?;
    namespace.declare("select", Cell::Command(Command::new(select::select)))?;
    namespace.declare("enumerate", Cell::Command(Command::new(enumerate::enumerate)))?;

    namespace.declare("cast", Cell::Command(Command::new(cast::cast)))?;

    namespace.declare("head", Cell::Command(Command::new(head::head)))?;
    namespace.declare("tail", Cell::Command(Command::new(tail::tail)))?;

    namespace.declare("lines", Cell::Command(Command::new(lines::lines)))?;
    namespace.declare("csv", Cell::Command(Command::new(csv::csv)))?;

    return Ok(());
}
