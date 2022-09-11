use crate::lang::errors::to_crush_error;
use crate::lang::execute;
use crate::lang::pipe::ValueSender;
use crate::lang::{errors::CrushResult, data::scope::Scope};
use std::fs::read_dir;
use std::path::Path;
use crate::lang::global_state::GlobalState;

#[macro_use]
pub mod binary_op;

mod comp;
mod cond;
mod constants;
mod control;
mod crush;
#[cfg(target_os = "linux")]
mod dbus;
mod dns;
mod fd;
mod fs;
mod grpc;
mod host;
mod io;
mod math;
mod random;
mod remote;
mod stream;
#[cfg(target_os = "linux")]
mod systemd;
mod term;
pub mod types;
mod user;
mod var;

fn declare_external(
    root: &Scope,
    global_state: &GlobalState,
    output: &ValueSender,
) -> CrushResult<()> {
    match read_dir("src/crushlib/") {
        Err(_) => Ok(()),
        Ok(dirs) => {
            for lib in dirs {
                match lib {
                    Ok(entry) => match entry.file_name().to_str() {
                        None => {
                            global_state.printer().error("Invalid filename encountered during library loading");
                        }
                        Some(name_with_extension) => {
                            let name = name_with_extension.trim_end_matches(".crush");
                            let s = load_external_namespace(
                                name,
                                &entry.path(),
                                root,
                                global_state,
                                output,
                            )?;
                            if name == "lls" {
                                root.r#use(&s);
                            }
                        }
                    },
                    err => global_state.printer().handle_error(to_crush_error(err)),
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
    global_state: &GlobalState,
    output: &ValueSender,
) -> CrushResult<Scope> {
    let local_output = output.clone();
    let local_file = file.to_path_buf();
    let local_state = global_state.clone();
    root.create_namespace(
        name,
        "",
        Box::new(move |env| {
            let tmp_env: Scope = env.create_temporary_namespace();
            execute::file(
                &tmp_env,
                &local_file,
                &local_output,
                &local_state)?;
            let data = tmp_env.export()?;
            for (k, v) in data.mapping {
                env.declare(&k, v)?;
            }
            Ok(())
        }),
    )
}

pub fn declare(
    root: &Scope,
    global_state: &GlobalState,
    output: &ValueSender,
) -> CrushResult<()> {
    comp::declare(root)?;
    cond::declare(root)?;
    constants::declare(root)?;
    control::declare(root)?;
    crush::declare(root)?;
    #[cfg(target_os = "linux")]
        dbus::declare(root)?;
    dns::declare(root)?;
    fd::declare(root)?;
    fs::declare(root)?;
    grpc::declare(root)?;
    host::declare(root)?;
    io::declare(root)?;
    math::declare(root)?;
    random::declare(root)?;
    remote::declare(root)?;
    stream::declare(root)?;
    #[cfg(target_os = "linux")]
        systemd::declare(root)?;
    term::declare(root)?;
    types::declare(root)?;
    user::declare(root)?;
    var::declare(root)?;

    declare_external(root, global_state, output)?;

    root.readonly();
    Ok(())
}
