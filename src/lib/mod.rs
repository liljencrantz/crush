mod command_util;

pub mod parse_util;

mod r#struct;
mod val;

mod file;

mod var;

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

mod uniq;
mod group;
mod join;
mod zip;

//mod aggr;

mod count;
mod sum;

mod cat;
mod materialize;
mod http;

mod r#for;
mod r#if;

mod r#type;
mod list;
mod dict;
mod time;
mod math;
mod comp;

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
use crate::stream::{ValueReceiver, ValueSender, InputStream};
use crate::data::ValueType;

pub struct ExecutionContext {
    pub input: ValueReceiver,
    pub output: ValueSender,
    pub arguments: Vec<Argument>,
    pub env: Env,
    pub printer: Printer,
}

pub struct StreamExecutionContext {
    pub argument_stream: InputStream,
    pub output: ValueSender,
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

pub fn declare(root: &Env) -> CrushResult<()> {
    root.declare_str("true", Value::Bool(true))?;
    root.declare_str("false", Value::Bool(false))?;
    root.declare_str("global", Value::Env(root.clone()))?;

    root.declare_str("struct", Value::Command(Command::new(r#struct::perform)))?;
    root.declare_str("val", Value::Command(Command::new(val::perform)))?;
    root.declare_str("materialize", Value::Command(Command::new(materialize::perform)))?;


    root.declare_str("ps", Value::Command(Command::new(ps::perform)))?;

    root.declare_str("head", Value::Command(Command::new(head::perform)))?;
    root.declare_str("tail", Value::Command(Command::new(tail::perform)))?;
    root.declare_str("where", Value::Command(Command::new(r#where::perform)))?;
    root.declare_str("sort", Value::Command(Command::new(sort::perform)))?;
    root.declare_str("reverse", Value::Command(Command::new(reverse::perform)))?;
    root.declare_str("group", Value::Command(Command::new(group::perform)))?;
    root.declare_str("join", Value::Command(Command::new(join::perform)))?;
    root.declare_str("uniq", Value::Command(Command::new(uniq::perform)))?;
//    env.declare_str("aggr", Value::Command(Command::new(aggr::perform)))?;
    root.declare_str("count", Value::Command(Command::new(count::perform)))?;
    root.declare_str("sum", Value::Command(Command::new(sum::perform)))?;
    root.declare_str("select", Value::Command(Command::new(select::perform)))?;
    root.declare_str("enumerate", Value::Command(Command::new(enumerate::perform)))?;
    root.declare_str("zip", Value::Command(Command::new(zip::perform)))?;

    root.declare_str("cat", Value::Command(Command::new(cat::perform)))?;
    root.declare_str("http", Value::Command(Command::new(http::perform)))?;
    root.declare_str("lines", Value::Command(Command::new(lines::perform)))?;
    root.declare_str("csv", Value::Command(Command::new(csv::perform)))?;
    root.declare_str("json", Value::Command(Command::new(json::perform)))?;

    root.declare_str("if", Value::Command(Command::new(r#if::perform)))?;
    root.declare_str("for", Value::Command(Command::new(r#for::perform)))?;

    list::declare(root)?;
    dict::declare(root)?;
    r#type::declare(root)?;
    time::declare(root)?;
    math::declare(root)?;
    comp::declare(root)?;
    file::declare(root)?;
    var::declare(root)?;

    return Ok(());
}
