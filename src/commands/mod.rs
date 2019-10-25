mod command_util;

mod find;
mod echo;

mod pwd;
mod ps;

mod cd;

mod set;
mod lett;

mod unset;

mod head;
mod tail;

mod lines;
mod csv;

mod r#where;
mod sort;
mod reverse;
mod select;
mod enumerate;
mod group;
mod join;
mod count;
mod cat;

mod cast;

mod forr;

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
use crate::data::{ColumnType, CellType, JobOutput};
use crate::job::Job;
use std::sync::{Arc, Mutex};
use crate::closure::Closure;
use crate::errors::JobResult;

type CommandInvocation = Box<FnOnce() -> JobResult<()> + Send>;

pub enum Exec {
    Closure(Closure),
    Command(CommandInvocation),
}

pub enum JobJoinHandle {
    Many(Vec<JobJoinHandle>),
    Async(JoinHandle<JobResult<()>>),
}

pub struct CompileContext {
    pub input_type: Vec<ColumnType>,
    pub input: InputStream,
    pub output: OutputStream,
    pub arguments: Vec<Argument>,
    pub env: Env,
    pub printer: Printer,
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


pub struct Call {
    name: String,
    output_type: Vec<ColumnType>,
    exec: Exec,
    printer: Printer,
    env: Env,
}

impl Call {
    pub fn new(
        name: String,
        output_type: Vec<ColumnType>,
        exec: Exec,
        printer: Printer,
        env: Env,
    ) -> Call {
        Call {
            name,
            output_type,
            exec,
            printer,
            env,
        }
    }

    pub fn get_name(&self) -> &String {
        return &self.name;
    }

    pub fn get_output_type(&self) -> &Vec<ColumnType> {
        return &self.output_type;
    }

    pub fn execute(mut self) -> JobJoinHandle {
        let env = self.env.clone();
        let printer = self.printer.clone();
        let name = self.name.clone();

        match self.exec {
            Exec::Closure(closure) => closure.execute(),
            Exec::Command(cmd) => handle(build(name).spawn(move || cmd())),
        }
    }
}

fn build(name: String) -> thread::Builder {
    thread::Builder::new().name(name)
}

fn handle(h: Result<JoinHandle<JobResult<()>>, std::io::Error>) -> JobJoinHandle {
    JobJoinHandle::Async(h.unwrap())
}

pub fn add_builtins(env: &Env) -> JobResult<()> {
    env.declare("ls", Cell::Command(Command::new(find::compile_ls)))?;
    env.declare("find", Cell::Command(Command::new(find::compile_find)))?;
    env.declare("echo", Cell::Command(Command::new(echo::compile)))?;

    env.declare("ps", Cell::Command(Command::new(ps::compile)))?;

    env.declare("pwd", Cell::Command(Command::new(pwd::compile)))?;
    env.declare("cd", Cell::Command(Command::new(cd::compile)))?;
    env.declare("where", Cell::Command(Command::new(r#where::compile)))?;
    env.declare("sort", Cell::Command(Command::new(sort::compile)))?;
    env.declare("reverse", Cell::Command(Command::new(reverse::compile)))?;
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

    env.declare("for", Cell::Command(Command::new(forr::compile)))?;

    return Ok(());
}
