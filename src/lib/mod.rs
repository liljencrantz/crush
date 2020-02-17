mod command_util;

pub mod parse_util;

mod r#struct;
mod val;

mod file;

mod var;

mod ps;


mod lines;
mod csv;
mod json;


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
mod stream;

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
    stream::declare(root)?;

    return Ok(());
}
