use crate::lang::errors::to_crush_error;
use crate::lang::execute;
use crate::lang::pipe::ValueSender;
use crate::lang::errors::CrushResult;
use std::fs::read_dir;
use std::path::Path;
use crate::lang::state::global_state::GlobalState;
use crate::lang::state::scope::Scope;

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

pub fn declare(root: &Scope) -> CrushResult<()> {
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

    root.read_only();
    Ok(())
}
