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
    Filter(r#where::Config),
    Sort(sort::Config),
    Select(select::Config),
    Enumerate(enumerate::Config),
    Group(group::Config),
    Join(join::Config),
    Count(count::Config),
    Cat(cat::Config),
    Ps(ps::Config),
    Reverse(reverse::Config),
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
            Filter(config) => handle(build(name).spawn(move || r#where::run(config, env, printer))),
            Sort(config) => handle(build(name).spawn(move || sort::run(config, env, printer))),
            Select(config) => handle(build(name).spawn(move || select::run(config, env, printer))),
            Enumerate(config) => handle(build(name).spawn(move || enumerate::run(config, env, printer))),
            Group(config) => handle(build(name).spawn(move || group::run(config, env, printer))),
            Join(config) => handle(build(name).spawn(move || join::run(config, env, printer))),
            Count(config) => handle(build(name).spawn(move || count::run(config, env, printer))),
            Cat(config) => handle(build(name).spawn(move || cat::run(config, env, printer))),
            Ps(config) => handle(build(name).spawn(move || ps::run(config, env, printer))),
            Reverse(config) => handle(build(name).spawn(move || reverse::run(config, env, printer))),
        }
    }
}

fn build(name: String) -> thread::Builder {
    thread::Builder::new().name(name)
}

fn handle(h: Result<JoinHandle<Result<(), JobError>>, std::io::Error>) -> JobJoinHandle {
    JobJoinHandle::Async(h.unwrap())
}

pub fn add_builtins(env: &Env) -> Result<(), JobError> {
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

    return Ok(());
}
