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
        BaseArgument,
        ArgumentDefinition,
        Command,
        Cell
    },
    stream::{InputStream, OutputStream},
};
use std::thread::JoinHandle;
use std::error::Error;
use crate::printer::Printer;
use crate::data::{CellDefinition, ConcreteCell};
use crate::job::Job;

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


pub struct BaseCall<C> {
    name: String,
    input_type: Vec<CellType>,
    arguments: Vec<BaseArgument<C>>,
    output_type: Vec<CellType>,
    exec: Exec,
}

#[derive(Clone)]
pub struct CallDefinition {
    pub arguments: Vec<ArgumentDefinition>,
    pub command: Command,
}

impl CallDefinition {
    pub fn call(&self, input_type: Vec<CellType>, dependencies: &mut Vec<Job>) -> Result<Call, JobError> {
        let c = self.command.call;
        let mut args: Vec<Argument> = Vec::new();
        for arg in self.arguments.iter() {
          args.push(arg.argument(dependencies)?);
        }

        return c(input_type, args);
    }
}

pub type Call = BaseCall<Cell>;

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

pub fn add_builtins(namespace: &mut Namespace) -> Result<(), JobError> {
    namespace.declare("ls", ConcreteCell::Command(Command::new(ls_and_find::ls)))?;
    namespace.declare("find", ConcreteCell::Command(Command::new(ls_and_find::find)))?;
    namespace.declare("echo", ConcreteCell::Command(Command::new(echo::echo)))?;
    namespace.declare("pwd", ConcreteCell::Command(Command::new(pwd::pwd)))?;
    namespace.declare("cd", ConcreteCell::Command(Command::new(cd::cd)))?;
    namespace.declare("filter", ConcreteCell::Command(Command::new(filter::filter)))?;
    namespace.declare("sort", ConcreteCell::Command(Command::new(sort::sort)))?;
    namespace.declare("set", ConcreteCell::Command(Command::new(set::set)))?;
    namespace.declare("let", ConcreteCell::Command(Command::new(let_command::let_command)))?;
    namespace.declare("unset", ConcreteCell::Command(Command::new(unset::unset)))?;
    namespace.declare("group", ConcreteCell::Command(Command::new(group::group)))?;
    namespace.declare("join", ConcreteCell::Command(Command::new(join::join)))?;
    namespace.declare("count", ConcreteCell::Command(Command::new(count::count)))?;
    namespace.declare("cat", ConcreteCell::Command(Command::new(cat::cat)))?;
    namespace.declare("select", ConcreteCell::Command(Command::new(select::select)))?;
    namespace.declare("enumerate", ConcreteCell::Command(Command::new(enumerate::enumerate)))?;

    namespace.declare("cast", ConcreteCell::Command(Command::new(cast::cast)))?;

    namespace.declare("head", ConcreteCell::Command(Command::new(head::head)))?;
    namespace.declare("tail", ConcreteCell::Command(Command::new(tail::tail)))?;

    namespace.declare("lines", ConcreteCell::Command(Command::new(lines::lines)))?;
    namespace.declare("csv", ConcreteCell::Command(Command::new(csv::csv)))?;

    return Ok(());
}
