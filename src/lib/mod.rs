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
mod text;
mod control;
mod constants;

use crate::{
    lang::scope::Scope,
    lang::{
        argument::Argument,
        command::SimpleCommand,
        value::Value,
    },
};
use std::thread::{JoinHandle};
use crate::lang::printer::Printer;
use crate::errors::CrushResult;
use crate::lang::stream::{ValueReceiver, ValueSender, InputStream};

pub use control::cmd;

pub fn declare(root: &Scope) -> CrushResult<()> {
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
    text::declare(root)?;
    constants::declare(root)?;
    root.readonly();
    return Ok(());
}
