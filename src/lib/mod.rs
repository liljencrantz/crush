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
use crate::lang::execute;
use crate::lang::stream::ValueSender;
use crate::lang::printer::Printer;
use std::path::PathBuf;

pub fn declare_non_native(root: &Scope, printer: &Printer, output: &ValueSender) -> CrushResult<()> {
    let local_printer = printer.clone();
    let local_output = output.clone();
    root.create_lazy_namespace("lll", Box::new(move |env| {
        let tmp_env: Scope = env.parent().create_temporary_namespace("<tmp>")?;
        execute::file(tmp_env.clone(), &PathBuf::from("src/l/ll.crush"), &local_printer, &local_output)?;
        let data = tmp_env.export()?;
        for (k,v) in data.mapping {
            env.declare(&k, v)?;
        }
        Ok(())
    }))?;
    Ok(())
}

pub fn declare(root: &Scope, printer: &Printer, output: &ValueSender) -> CrushResult<()> {
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

    declare_non_native(root, printer, output)?;

    root.readonly();
    Ok(())
}
