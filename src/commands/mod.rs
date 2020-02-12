mod command_util;

pub mod parse_util;

mod r#struct;
mod val;

mod find;
mod stat;
mod cd;
mod pwd;

mod set;
mod r#let;
mod unset;
mod env;

mod ps;

mod head;
mod tail;

mod lines;
mod csv;
mod json;

mod r#where;
mod sort;
mod reverse;

mod field;
mod select;
mod enumerate;

mod uniq;
mod group;
mod join;
mod zip;

//mod aggr;

mod count;
mod sum;

mod cat;
mod http;

mod r#type;

mod list;
mod dict;

mod time;

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
use crate::errors::CrushResult;
use crate::stream::{ValueReceiver, ValueSender};
use crate::data::ValueType;

pub struct CompileContext {
    pub input: ValueReceiver,
    pub output: ValueSender,
    pub arguments: Vec<Argument>,
    pub env: Env,
    pub printer: Printer,
}

pub enum JobJoinHandle {
    Many(Vec<JobJoinHandle>),
    Async(JoinHandle<CrushResult<()>>),
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

pub fn add_commands(env: &Env) -> CrushResult<()> {
    env.declare_str("struct", Value::Command(Command::new(r#struct::perform)))?;
    env.declare_str("val", Value::Command(Command::new(val::perform)))?;

    env.declare_str("ls", Value::Command(Command::new(find::perform_ls)))?;
    env.declare_str("find", Value::Command(Command::new(find::perform_find)))?;
    env.declare_str("stat", Value::Command(Command::new(stat::perform)))?;
    env.declare_str("cd", Value::Command(Command::new(cd::perform)))?;
    env.declare_str("pwd", Value::Command(Command::new(pwd::perform)))?;

    env.declare_str("let", Value::Command(Command::new(r#let::perform)))?;
    env.declare_str("set", Value::Command(Command::new(set::perform)))?;
    env.declare_str("unset", Value::Command(Command::new(unset::perform)))?;
    env.declare_str("env", Value::Command(Command::new(env::perform)))?;

    env.declare_str("ps", Value::Command(Command::new(ps::perform)))?;

    env.declare_str("head", Value::Command(Command::new(head::perform)))?;
    env.declare_str("tail", Value::Command(Command::new(tail::perform)))?;

    env.declare_str("where", Value::Command(Command::new(r#where::perform)))?;
    env.declare_str("sort", Value::Command(Command::new(sort::perform)))?;
    env.declare_str("reverse", Value::Command(Command::new(reverse::perform)))?;

    env.declare_str("group", Value::Command(Command::new(group::perform)))?;
    env.declare_str("join", Value::Command(Command::new(join::perform)))?;
    env.declare_str("uniq", Value::Command(Command::new(uniq::perform)))?;

//    env.declare_str("aggr", Value::Command(Command::new(aggr::perform)))?;

    env.declare_str("count", Value::Command(Command::new(count::perform)))?;
    env.declare_str("sum", Value::Command(Command::new(sum::perform)))?;
    env.declare_str("cat", Value::Command(Command::new(cat::perform)))?;
    env.declare_str("http", Value::Command(Command::new(http::perform)))?;

    env.declare_str("field", Value::Command(Command::new(field::perform)))?;
    env.declare_str("select", Value::Command(Command::new(select::perform)))?;
    env.declare_str("enumerate", Value::Command(Command::new(enumerate::perform)))?;

    env.declare_str("lines", Value::Command(Command::new(lines::perform)))?;
    env.declare_str("csv", Value::Command(Command::new(csv::perform)))?;
    env.declare_str("json", Value::Command(Command::new(json::perform)))?;

    env.declare_str("if", Value::Command(Command::new(r#if::perform)))?;
    env.declare_str("for", Value::Command(Command::new(r#for::perform)))?;
    env.declare_str("zip", Value::Command(Command::new(zip::perform)))?;

    let list = env.create_namespace("list")?;

    list.declare_str("of", Value::Command(Command::new(list::of)))?;
    list.declare_str("create", Value::Command(Command::new(list::create)))?;
    list.declare_str("len", Value::Command(Command::new(list::len)))?;
    list.declare_str("empty", Value::Command(Command::new(list::empty)))?;
    list.declare_str("push", Value::Command(Command::new(list::push)))?;
    list.declare_str("pop", Value::Command(Command::new(list::pop)))?;


    dict::declare(env)?;
    r#type::declare(env)?;
    time::declare(env)?;

    return Ok(());
}
