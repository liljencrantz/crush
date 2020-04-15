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
pub mod math;
mod toml;
mod json;

use crate::{lang::scope::Scope, lang::errors::CrushResult};

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
    math::declare(root)?;
    toml::declare(root)?;
    json::declare(root)?;
    root.readonly();
    Ok(())
}
