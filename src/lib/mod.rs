pub mod traversal;
pub mod var;
pub mod proc;
pub mod input;

#[macro_use]
pub mod binary_op;

mod comp;
mod cond;
mod stream;
pub mod types;
mod control;
mod constants;
mod math;
mod toml;
mod json;
mod user;

use crate::{lang::scope::Scope, lang::errors::CrushResult};

pub fn declare_non_native(root: &Scope) -> CrushResult<()> {
    Ok(())
}

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
    user::declare(root)?;

    declare_non_native(root)?;

    root.readonly();
    Ok(())
}
