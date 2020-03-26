mod command_util;
mod parse_util;

pub mod traversal;
pub mod var;
pub mod proc;
pub mod input;

#[macro_use]
pub mod binary_op;

pub mod comp;
pub mod cond;
pub mod stream;
pub mod types;
pub mod control;
pub mod constants;

use crate::{
    lang::argument::Argument,
    lang::scope::Scope,
    lang::value::Value,
    lang::errors::CrushResult,
    lang::stream::{ValueReceiver, ValueSender, InputStream}
};
use std::thread::JoinHandle;

pub use control::cmd;

pub fn declare(root: &Scope) -> CrushResult<()> {
    comp::declare(root)?;
    cond::declare(root)?;
    traversal::declare(root)?;
    var::declare(root)?;
    stream::declare(root)?;
    types::declare(root)?;
    proc::declare(root)?;
    input::declare(root)?;
    control::declare(root)?;
    constants::declare(root)?;
    root.readonly();
    return Ok(());
}
