use crate::lang::errors::CrushResult;
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

pub fn declare(root: &Scope) -> CrushResult<()> {
    comp::declare(root)?;
    cond::declare(root)?;
    constants::declare(root)?;
    control::declare(root)?;
    crush::declare(root)?;
    #[cfg(target_os = "linux")]
        dbus::declare(root)?;
    dns::declare(root)?;
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
