mod command_util;

pub mod parse_util;

mod file;
mod var;
mod proc;
mod io;

mod r#type;
mod time;
mod math;
mod comp;
mod stream;
mod data;
mod control;

use crate::{
    namespace::Namespace,
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
    pub env: Namespace,
    pub printer: Printer,
    pub is_loop: bool,
}

pub struct StreamExecutionContext {
    pub argument_stream: InputStream,
    pub output: ValueSender,
    pub env: Namespace,
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

pub fn declare(root: &Namespace) -> CrushResult<()> {
    root.declare_str("true", Value::Bool(true))?;
    root.declare_str("false", Value::Bool(false))?;
    root.declare_str("global", Value::Env(root.clone()))?;

    r#type::declare(root)?;
    time::declare(root)?;
    math::declare(root)?;
    comp::declare(root)?;
    file::declare(root)?;
    var::declare(root)?;
    stream::declare(root)?;
    data::declare(root)?;
    proc::declare(root)?;
    io::declare(root)?;
    control::declare(root)?;

    return Ok(());
}
