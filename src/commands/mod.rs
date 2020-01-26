mod command_util;

mod r#struct;

mod find;
mod stat;
mod cd;
mod pwd;

mod set;
mod r#let;
mod unset;
mod env;
mod take;

mod ps;

mod head;
mod tail;

mod lines;
mod csv;
mod json;

mod r#where;
mod sort;
mod reverse;

mod select;
mod enumerate;

mod group;
mod join;
mod zip;

//mod aggr;

mod count;
mod sum;

mod cat;

mod cast;

mod list;
mod dict;

mod r#for;
mod r#if;

use crate::{
    env::Env,
    data::{
        Argument,
        Command,
        Value,
    },
};
use std::thread::{JoinHandle};
use crate::printer::Printer;
use crate::errors::JobResult;
use crate::stream::{ValueReceiver, ValueSender};

pub struct CompileContext {
    pub input: ValueReceiver,
    pub output: ValueSender,
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

pub fn add_commands(env: &Env) -> JobResult<()> {
    env.declare_str("struct", Value::Command(Command::new(r#struct::compile_and_run)))?;

    env.declare_str("ls", Value::Command(Command::new(find::compile_and_run_ls)))?;
    env.declare_str("find", Value::Command(Command::new(find::compile_and_run_find)))?;
    env.declare_str("stat", Value::Command(Command::new(stat::compile_and_run)))?;
    env.declare_str("cd", Value::Command(Command::new(cd::compile_and_run)))?;
    env.declare_str("pwd", Value::Command(Command::new(pwd::compile_and_run)))?;

    env.declare_str("let", Value::Command(Command::new(r#let::compile_and_run)))?;
    env.declare_str("set", Value::Command(Command::new(set::compile_and_run)))?;
    env.declare_str("unset", Value::Command(Command::new(unset::compile_and_run)))?;
    env.declare_str("env", Value::Command(Command::new(env::compile_and_run)))?;
    env.declare_str("take", Value::Command(Command::new(take::compile_and_run)))?;

    env.declare_str("ps", Value::Command(Command::new(ps::compile_and_run)))?;

    env.declare_str("head", Value::Command(Command::new(head::compile_and_run)))?;
    env.declare_str("tail", Value::Command(Command::new(tail::compile_and_run)))?;

    env.declare_str("where", Value::Command(Command::new(r#where::compile_and_run)))?;
    env.declare_str("sort", Value::Command(Command::new(sort::compile_and_run)))?;
    env.declare_str("reverse", Value::Command(Command::new(reverse::compile_and_run)))?;

    env.declare_str("group", Value::Command(Command::new(group::compile_and_run)))?;
    env.declare_str("join", Value::Command(Command::new(join::compile_and_run)))?;

//    env.declare_str("aggr", Value::Command(Command::new(aggr::compile_and_run)))?;

    env.declare_str("count", Value::Command(Command::new(count::compile_and_run)))?;
    env.declare_str("sum", Value::Command(Command::new(sum::compile_and_run)))?;
    env.declare_str("cat", Value::Command(Command::new(cat::compile_and_run)))?;
    env.declare_str("select", Value::Command(Command::new(select::compile_and_run)))?;
    env.declare_str("enumerate", Value::Command(Command::new(enumerate::compile_and_run)))?;

    env.declare_str("cast", Value::Command(Command::new(cast::compile_and_run)))?;

    env.declare_str("lines", Value::Command(Command::new(lines::compile_and_run)))?;
    env.declare_str("csv", Value::Command(Command::new(csv::compile_and_run)))?;
    env.declare_str("json", Value::Command(Command::new(json::compile_and_run)))?;

    env.declare_str("if", Value::Command(Command::new(r#if::compile_and_run)))?;
    env.declare_str("for", Value::Command(Command::new(r#for::compile_and_run)))?;
    env.declare_str("zip", Value::Command(Command::new(zip::compile_and_run)))?;

    let list = env.create_namespace("list")?;

    list.declare_str("create", Value::Command(Command::new(list::create)))?;
    list.declare_str("len", Value::Command(Command::new(list::len)))?;
    list.declare_str("empty", Value::Command(Command::new(list::empty)))?;
    list.declare_str("push", Value::Command(Command::new(list::push)))?;
    list.declare_str("pop", Value::Command(Command::new(list::pop)))?;

    let dict = env.create_namespace("dict")?;

    dict.declare_str("create", Value::Command(Command::new(dict::create)))?;
    dict.declare_str("insert", Value::Command(Command::new(dict::insert)))?;
    dict.declare_str("get", Value::Command(Command::new(dict::get)))?;
    dict.declare_str("remove", Value::Command(Command::new(dict::remove)))?;
    dict.declare_str("len", Value::Command(Command::new(dict::len)))?;
    dict.declare_str("empty", Value::Command(Command::new(dict::empty)))?;

    return Ok(());
}
