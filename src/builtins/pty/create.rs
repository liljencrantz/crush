use crate::builtins::pty::methods::methods;
use crate::builtins::pty::session::PtySession;
use crate::lang::data::r#struct::Struct;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::signature::text::Text;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use signature::signature;

#[signature(
    pty.create,
    can_block = true,
    short = "Spawn a command attached to a fresh virtual terminal, for driving and inspecting a real interactive session from a script.",
    long = "Returns a struct with one method per operation (`write`, `wait_for`, `contents`,",
    long = "`close`, plus `pid`/`alive`). Always call `close` once done, to release the",
    long = "underlying process. For control characters (e.g. Ctrl-Z), use the module-level",
    long = "`pty:control` constants (see `pty:control`), not a field on this struct.",
    example = "$t := $(pty:create crush)",
    example = "$t:write \"echo hello\\n\"",
    example = "$t:wait_for \"hello\"",
    example = "$t:write $pty:control:z",
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
    let rows = terminal_dimension("rows", cfg.rows)?;
    let cols = terminal_dimension("cols", cfg.cols)?;
    let id = PtySession::create(&cfg.command.as_path(), &cfg.arguments, rows, cols)?;

    let mut fields = vec![];
    for name in [
        "write", "wait_for", "contents", "pid", "alive", "close",
    ] {
        fields.push((name, method_struct(id, name)?));
    }

    context.output.send(Value::Struct(Struct::new(fields, None)))
}

/// Validates a terminal dimension: the pty and the screen emulator both need it to be a
/// positive `u16`, and a silently wrapped value (or `0`) would otherwise surface only
/// as an opaque failure deep inside the session.
fn terminal_dimension(name: &str, value: i128) -> CrushResult<u16> {
    match u16::try_from(value) {
        Ok(v) if v >= 1 => Ok(v),
        _ => command_error(format!(
            "`{}` must be between 1 and {}, got {}",
            name,
            u16::MAX,
            value
        )),
    }
}

fn method_struct(id: i32, name: &str) -> CrushResult<Value> {
    let cmd = match methods().get(name) {
        Some(cmd) => cmd.clone(),
        None => return command_error(format!("Unknown pty method `{}`", name)),
    };
    Ok(Value::Struct(Struct::new(
        vec![("id", Value::from(id)), ("__call__", Value::Command(cmd))],
        None,
    )))
}
