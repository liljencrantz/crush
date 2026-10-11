use crate::CrushResult;
use crate::builtins::pty::control::control_value;
use crate::builtins::pty::create::Create;
use crate::lang::state::scope::Scope;

mod control;
mod create;
mod methods;
mod session;

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "pty",
        "Pseudo-terminal sessions, for driving and inspecting a real interactive session from a script",
        None,
        Box::new(move |env| {
            Create::declare(env)?;
            env.declare("control", control_value().clone())?;
            Ok(())
        }),
    )?;
    Ok(())
}

/// Called exactly once, unconditionally, near the end of `main.rs`'s `run()` -- see
/// src/builtins/pty/session.rs's `PtySession::close_all` doc comment for why.
pub fn close_all_sessions() {
    session::PtySession::close_all();
}
