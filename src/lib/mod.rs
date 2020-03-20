mod command_util;
mod parse_util;

pub mod file;
pub mod var;
pub mod proc;
pub mod io;
pub mod r#type;
pub mod time;
pub mod math;
pub mod comp;
pub mod cond;
pub mod stream;
pub mod data;
pub mod text;
pub mod control;
pub mod constants;

use crate::{
    lang::argument::Argument,
    lang::scope::Scope,
    lang::command::SimpleCommand,
    lang::value::Value,
    lang::errors::CrushResult,
    lang::stream::{ValueReceiver, ValueSender, InputStream}
};
use std::thread::JoinHandle;

pub use control::cmd;

pub fn declare(root: &Scope) -> CrushResult<()> {
    r#type::declare(root)?;
    time::declare(root)?;
    math::declare(root)?;
    comp::declare(root)?;
    cond::declare(root)?;
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
