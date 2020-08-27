use crate::lang::errors::to_crush_error;
use crate::lang::execute;
use crate::lang::printer::Printer;
use crate::lang::stream::ValueSender;
use crate::{lang::errors::CrushResult, lang::scope::Scope};
use std::fs::read_dir;
use std::path::Path;
use crate::lang::threads::ThreadStore;

#[macro_use]
pub mod binary_op;

mod comp;
mod cond;
mod constants;
mod control;
#[cfg(target_os = "linux")]
mod dbus;
mod host;
mod io;
mod math;
#[cfg(target_os = "linux")]
mod proc;
mod random;
mod remote;
mod stream;
#[cfg(target_os = "linux")]
mod systemd;
mod traversal;
pub mod types;
mod user;
mod var;

fn declare_external(root: &Scope, printer: &Printer, threads: &ThreadStore, output: &ValueSender) -> CrushResult<()> {
    match read_dir("src/crushlib/") {
        Err(_) => Ok(()),
        Ok(dirs) => {
            for lib in dirs {
                match lib {
                    Ok(entry) => match entry.file_name().to_str() {
                        None => {
                            printer.error("Invalid filename encountered during library loading");
                        }
                        Some(name_with_extension) => {
                            let name = name_with_extension.trim_end_matches(".crush");
                            let s = load_external_namespace(
                                name,
                                &entry.path(),
                                root,
                                printer,
                                threads,
                                output,
                            )?;
                            if name == "lls" {
                                root.r#use(&s);
                            }
                        }
                    },
                    err => printer.handle_error(to_crush_error(err)),
                }
            }
            Ok(())
        }
    }
}

fn load_external_namespace(
    name: &str,
    file: &Path,
    root: &Scope,
    printer: &Printer,
    threads: &ThreadStore,
    output: &ValueSender,
) -> CrushResult<Scope> {
    let local_printer = printer.clone();
    let local_threads = threads.clone();
    let local_output = output.clone();
    let local_file = file.to_path_buf();
    root.create_namespace(
        name,
        Box::new(move |env| {
            let tmp_env: Scope = env.create_temporary_namespace();
            execute::file(tmp_env.clone(), &local_file, &local_printer, &local_output, &local_threads)?;
            let data = tmp_env.export()?;
            for (k, v) in data.mapping {
                env.declare(&k, v)?;
            }
            Ok(())
        }),
    )
}

pub fn declare(root: &Scope, printer: &Printer, threads: &ThreadStore, output: &ValueSender) -> CrushResult<()> {
    comp::declare(root)?;
    cond::declare(root)?;
    traversal::declare(root)?;
    var::declare(root)?;
    stream::declare(root)?;
    types::declare(root)?;
    #[cfg(target_os = "linux")]
    proc::declare(root)?;
    io::declare(root)?;
    control::declare(root)?;
    constants::declare(root)?;
    math::declare(root)?;
    user::declare(root)?;
    remote::declare(root)?;
    random::declare(root)?;
    host::declare(root)?;
    #[cfg(target_os = "linux")]
    dbus::declare(root)?;
    #[cfg(target_os = "linux")]
    systemd::declare(root)?;
    declare_external(root, printer, threads, output)?;
    root.readonly();
    Ok(())
}
