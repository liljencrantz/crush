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
mod remote;
mod pup;

use crate::{lang::scope::Scope, lang::errors::CrushResult};
use crate::lang::execute;
use crate::lang::stream::ValueSender;
use crate::lang::printer::Printer;
use std::path::Path;
use std::fs::read_dir;
use crate::lang::errors::to_crush_error;

fn declare_external(root: &Scope, printer: &Printer, output: &ValueSender) -> CrushResult<()> {
    match read_dir("src/crushlib/") {
        Err(_) => Ok(()),
        Ok(dirs) => {
            for lib in dirs {
                match lib {
                    Ok(entry) => {
                        match entry.file_name().to_str() {
                            None => {
                                printer.error("Invalid filename encountered during library loading");
                            }
                            Some(name_with_extension) => {
                                let name = name_with_extension.trim_end_matches(".crush");
                                let s = load_external_namespace(name, &entry.path(), root, printer, output)?;
                                if name == "lls" {
                                    root.r#use(&s);
                                }
                            }
                        }
                    }
                    err => printer.handle_error(to_crush_error(err)),
                }
            }
            Ok(())
        }
    }
}

fn load_external_namespace(name: &str, file: &Path, root: &Scope, printer: &Printer, output: &ValueSender) -> CrushResult<Scope> {
    let local_printer = printer.clone();
    let local_output = output.clone();
    let local_file = file.to_path_buf();
    root.create_lazy_namespace(name, Box::new(move |env| {
        let tmp_env: Scope = env.create_temporary_namespace()?;
        execute::file(tmp_env.clone(), &local_file, &local_printer, &local_output)?;
        let data = tmp_env.export()?;
        for (k, v) in data.mapping {
            env.declare(&k, v)?;
        }
        Ok(())
    }))
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
    remote::declare(root)?;
    pup::declare(root)?;
    declare_external(root, printer, output)?;
    root.readonly();
    Ok(())
}
