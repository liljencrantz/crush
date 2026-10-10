use crate::CrushResult;
use crate::builtins::vt::create::Create;
use crate::lang::state::scope::Scope;

mod control;
mod create;
mod methods;
mod session;

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "vt",
        "Virtual terminals, for driving and inspecting a real interactive session from a script",
        None,
        Box::new(move |env| {
            Create::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}

/// Called exactly once, unconditionally, near the end of `main.rs`'s `run()` -- see
/// src/builtins/vt/session.rs's `VtSession::close_all` doc comment for why.
pub fn close_all_sessions() {
    session::VtSession::close_all();
}
