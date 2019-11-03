mod command_util;

mod echo;

mod find;
mod cd;
mod pwd;

mod set;
mod r#let;
mod unset;

mod ps;

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

mod aggr;

mod count;
mod sum;

mod cat;

mod cast;

mod r#for;

use crate::{
    env::Env,
    data::{
        Argument,
        Command,
        Cell,
    },
};
use std::thread::{JoinHandle};
use crate::printer::Printer;
use crate::errors::JobResult;
use crate::stream::{UninitializedInputStream, UninitializedOutputStream};

pub struct CompileContext {
    pub input: UninitializedInputStream,
    pub output: UninitializedOutputStream,
    pub arguments: Vec<Argument>,
    pub env: Env,
    pub printer: Printer,
}

pub enum JobJoinHandle {
    Many(Vec<JobJoinHandle>),
    Async(JoinHandle<JobResult<()>>),
}

impl JobJoinHandle {
    pub fn join(self, printer: &Printer) {
        return match self {
            JobJoinHandle::Async(a) => match a.join() {
                Ok(r) => match r {
                    Ok(_) => {}
                    Err(e) => printer.job_error(e),
                },
                Err(_) => printer.error("Unknown error while waiting for command to exit"),
            },
            JobJoinHandle::Many(v) => {
                for j in v {
                    j.join(printer);
                }
            }
        };
    }
}

pub fn add_builtins(env: &Env) -> JobResult<()> {
    env.declare("echo", Cell::Command(Command::new(echo::compile_and_run)))?;

    env.declare("ls", Cell::Command(Command::new(find::compile_and_run_ls)))?;
    env.declare("find", Cell::Command(Command::new(find::compile_and_run_find)))?;
    env.declare("cd", Cell::Command(Command::new(cd::compile_and_run)))?;
    env.declare("pwd", Cell::Command(Command::new(pwd::parse_and_run)))?;

    env.declare("let", Cell::Command(Command::new(r#let::compile_and_run)))?;
    env.declare("set", Cell::Command(Command::new(set::compile_and_run)))?;
    env.declare("unset", Cell::Command(Command::new(unset::compile_and_run)))?;

    env.declare("ps", Cell::Command(Command::new(ps::compile_and_run)))?;

    env.declare("head", Cell::Command(Command::new(head::compile_and_run)))?;
    env.declare("tail", Cell::Command(Command::new(tail::compile_and_run)))?;


    env.declare("where", Cell::Command(Command::new(r#where::compile_and_run)))?;
    env.declare("sort", Cell::Command(Command::new(sort::compile_and_run)))?;
    env.declare("reverse", Cell::Command(Command::new(reverse::compile_and_run)))?;

    env.declare("group", Cell::Command(Command::new(group::compile_and_run)))?;
    env.declare("join", Cell::Command(Command::new(join::compile_and_run)))?;


    env.declare("aggr", Cell::Command(Command::new(aggr::compile_and_run)))?;

    env.declare("count", Cell::Command(Command::new(count::compile_and_run)))?;
    env.declare("sum", Cell::Command(Command::new(sum::compile_and_run)))?;
    env.declare("cat", Cell::Command(Command::new(cat::compile_and_run)))?;
    env.declare("select", Cell::Command(Command::new(select::compile_and_run)))?;
    env.declare("enumerate", Cell::Command(Command::new(enumerate::compile_and_run)))?;

    env.declare("cast", Cell::Command(Command::new(cast::compile_and_run)))?;


    env.declare("lines", Cell::Command(Command::new(lines::compile_and_run)))?;
    env.declare("csv", Cell::Command(Command::new(csv::compile_and_run)))?;

    env.declare("for", Cell::Command(Command::new(r#for::compile_and_run)))?;

    return Ok(());
}
