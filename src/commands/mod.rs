mod command_util;

mod find;
mod echo;

mod pwd;

mod cd;

mod set;
mod lett;

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
    env::Env,
    data::{
        CellDefinition,
        Argument,
        BaseArgument,
        ArgumentDefinition,
        Command,
        Cell,
    },
    stream::{InputStream, OutputStream},
};
use std::thread::{JoinHandle, spawn};
use std::error::Error;
use crate::printer::Printer;
use crate::data::{CellFnurp, CellType, JobOutput};
use crate::job::Job;
use std::sync::{Arc, Mutex};
use crate::closure::Closure;

type CommandInvocation = fn(
    Vec<CellFnurp>,
    Vec<Argument>,
    InputStream,
    OutputStream,
    Env,
    Printer) -> Result<(), JobError>;

pub enum Exec {
    Closure(Closure),
    Pwd(pwd::Config),
    Echo(echo::Config),
    Let(lett::Config),
    Set(set::Config),
    Cd(cd::Config),
    Cast(cast::Config),
    Find(find::Config),
    Unset(unset::Config),
    Head(head::Config),
    Tail(tail::Config),
    Lines(lines::Config),
    Csv(csv::Config),
    Filter(filter::Config),
    Sort(sort::Config),
    Select(select::Config),
    Enumerate(enumerate::Config),
    Group(group::Config),
    Join(join::Config),
    Count(count::Config),
    Cat(cat::Config),
}

pub enum JobJoinHandle {
    Many(Vec<JobJoinHandle>),
    Async(JoinHandle<Result<(), JobError>>),
}

impl JobJoinHandle {
    pub fn join(self, printer: &Printer) {
        return match self {
            JobJoinHandle::Async(a) => match a.join() {
                Ok(r) => match r {
                    Ok(_) => {}
                    Err(e) => printer.job_error(e),
                },
                Err(e) => printer.error("Unknown error while waiting for command to exit"),
            },
            JobJoinHandle::Many(v) => {
                for j in v {
                    j.join(printer);
                }
            }
        };
    }
}

#[derive(Clone)]
#[derive(PartialEq)]
pub struct CallDefinition {
    name: String,
    arguments: Vec<ArgumentDefinition>,
}

impl CallDefinition {
    pub fn new(name: &str, arguments: Vec<ArgumentDefinition>) -> CallDefinition {
        CallDefinition {
            name: name.to_string(),
            arguments,
        }
    }

    pub fn compile(
        &self,
        env: &Env,
        printer: &Printer,
        input_type: Vec<CellFnurp>,
        input: InputStream,
        output: OutputStream,
        dependencies: &mut Vec<Job>,
    ) -> Result<Call, JobError> {
        let mut args: Vec<Argument> = Vec::new();
        for arg in self.arguments.iter() {
            args.push(arg.argument(dependencies, env, printer)?);
        }
        match &env.get(&self.name) {
            Some(Cell::Command(command)) => {
                let c = command.call;
                let (exec, output_type) = c(input_type, input, output, args)?;
                return Ok(Call {
                    name: self.name.clone(),
                    output_type,
                    exec,
                    printer: printer.clone(),
                    env: env.clone(),
                });
            }

            Some(Cell::ClosureDefinition(closure_definition)) => {
                let mut jobs: Vec<Job> = Vec::new();

                let closure = closure_definition.compile(env, printer, &input_type,
                                                         input, output,
                                                         args)?;
                let last_job = &closure.get_jobs()[closure.get_jobs().len() - 1];

                return Ok(Call {
                    name: self.name.clone(),
                    output_type: last_job.get_output_type().clone(),
                    exec: Exec::Closure(closure),
                    printer: printer.clone(),
                    env: env.clone(),
                });
            }
            _ => {
                return Err(error(format!("Unknown command name {}", &self.name).as_str()));
            }
        }
    }
}

pub struct Call {
    name: String,
    output_type: Vec<CellFnurp>,
    exec: Exec,
    printer: Printer,
    env: Env,
}

fn build(name: String) -> thread::Builder {
    thread::Builder::new().name(name)
}

fn handle(h: Result<JoinHandle<Result<(), JobError>>, std::io::Error>) -> JobJoinHandle {
    JobJoinHandle::Async(h.unwrap())
}

impl Call {
    pub fn get_name(&self) -> &String {
        return &self.name;
    }

    pub fn get_output_type(&self) -> &Vec<CellFnurp> {
        return &self.output_type;
    }


    pub fn execute(mut self) -> JobJoinHandle {
        let env = self.env.clone();
        let printer = self.printer.clone();
        let name = self.name.clone();
        use Exec::*;

        match self.exec {
            Closure(closure) => closure.execute(),
            Pwd(config) => handle(build(name).spawn(move || pwd::run(config, env, printer))),
            Echo(config) => handle(build(name).spawn(move || echo::run(config, env, printer))),
            Let(config) => handle(build(name).spawn(move || lett::run(config, env, printer))),
            Set(config) => handle(build(name).spawn(move || set::run(config, env, printer))),
            Cd(config) => handle(build(name).spawn(move || cd::run(config, env, printer))),
            Cast(config) => handle(build(name).spawn(move || cast::run(config, env, printer))),
            Find(config) => handle(build(name).spawn(move || find::run(config, env, printer))),
            Unset(config) => handle(build(name).spawn(move || unset::run(config, env, printer))),
            Head(config) => handle(build(name).spawn(move || head::run(config, env, printer))),
            Tail(config) => handle(build(name).spawn(move || tail::run(config, env, printer))),
            Lines(config) => handle(build(name).spawn(move || lines::run(config, env, printer))),
            Csv(config) => handle(build(name).spawn(move || csv::run(config, env, printer))),
            Filter(config) => handle(build(name).spawn(move || filter::run(config, env, printer))),
            Sort(config) => handle(build(name).spawn(move || sort::run(config, env, printer))),
            Select(config) => handle(build(name).spawn(move || select::run(config, env, printer))),
            Enumerate(config) => handle(build(name).spawn(move || enumerate::run(config, env, printer))),
            Group(config) => handle(build(name).spawn(move || group::run(config, env, printer))),
            Join(config) => handle(build(name).spawn(move || join::run(config, env, printer))),
            Count(config) => handle(build(name).spawn(move || count::run(config, env, printer))),
            Cat(config) => handle(build(name).spawn(move || cat::run(config, env, printer))),
        }
    }
}

pub fn add_builtins(env: &Env) -> Result<(), JobError> {
    env.declare("ls", Cell::Command(Command::new(find::compile_ls)))?;
    env.declare("find", Cell::Command(Command::new(find::compile_find)))?;
    env.declare("echo", Cell::Command(Command::new(echo::compile)))?;
    env.declare("pwd", Cell::Command(Command::new(pwd::compile)))?;
    env.declare("cd", Cell::Command(Command::new(cd::compile)))?;
    env.declare("filter", Cell::Command(Command::new(filter::compile)))?;
    env.declare("sort", Cell::Command(Command::new(sort::compile)))?;
    env.declare("set", Cell::Command(Command::new(set::compile)))?;
    env.declare("let", Cell::Command(Command::new(lett::compile)))?;
    env.declare("unset", Cell::Command(Command::new(unset::compile)))?;

    env.declare("group", Cell::Command(Command::new(group::compile)))?;
    env.declare("join", Cell::Command(Command::new(join::compile)))?;
    env.declare("count", Cell::Command(Command::new(count::compile)))?;
    env.declare("cat", Cell::Command(Command::new(cat::compile)))?;
    env.declare("select", Cell::Command(Command::new(select::compile)))?;
    env.declare("enumerate", Cell::Command(Command::new(enumerate::compile)))?;

    env.declare("cast", Cell::Command(Command::new(cast::compile)))?;

    env.declare("head", Cell::Command(Command::new(head::compile)))?;
    env.declare("tail", Cell::Command(Command::new(tail::compile)))?;

    env.declare("lines", Cell::Command(Command::new(lines::compile)))?;
    env.declare("csv", Cell::Command(Command::new(csv::compile)))?;

    return Ok(());
}
