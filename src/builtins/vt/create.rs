use crate::builtins::vt::control::control_value;
use crate::builtins::vt::methods::methods;
use crate::builtins::vt::session::VtSession;
use crate::lang::data::r#struct::Struct;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::signature::text::Text;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use signature::signature;

#[signature(
    vt.create,
    can_block = true,
    short = "Spawn a command attached to a fresh virtual terminal, for driving and inspecting a real interactive session from a script.",
    long = "Returns a struct with one method per operation (`write`, `wait_for`, `contents`,",
    long = "`close`, plus `pid`/`alive`) and a `control` struct of named control-character",
    long = "constants. Always call `close` once done, to release the underlying process.",
    example = "$t := $(vt:create crush)",
    example = "$t:write \"echo hello\\n\"",
    example = "$t:wait_for \"hello\"",
    example = "$t:close",
)]
pub struct Create {
    #[description("the program to run, e.g. `crush`.")]
    command: Text,
    #[unnamed()]
    #[description("arguments to pass to the program.")]
    arguments: Vec<String>,
    #[default(24)]
    #[description("the number of rows of the virtual terminal.")]
    rows: i128,
    #[default(80)]
    #[description("the number of columns of the virtual terminal.")]
    cols: i128,
}

fn create(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Create = Create::parse(context.remove_arguments(), &context.global_state.printer())?;
    let id = VtSession::create(
        &cfg.command.as_path(),
        &cfg.arguments,
        cfg.rows as u16,
        cfg.cols as u16,
    )?;

    let mut fields = vec![];
    for name in ["write", "wait_for", "contents", "pid", "alive", "close"] {
        fields.push((name, method_struct(id, name)?));
    }
    fields.push(("control", control_value().clone()));

    context.output.send(Value::Struct(Struct::new(fields, None)))
}

// `pub(super)` (not private) so Task 3 can reuse this when it extends the field list with
// `pid`/`alive`/`control`.
pub(super) fn method_struct(id: i32, name: &str) -> CrushResult<Value> {
    let cmd = match methods().get(name) {
        Some(cmd) => cmd.clone(),
        None => return command_error(format!("Unknown vt method `{}`", name)),
    };
    Ok(Value::Struct(Struct::new(
        vec![("id", Value::from(id)), ("__call__", Value::Command(cmd))],
        None,
    )))
}
