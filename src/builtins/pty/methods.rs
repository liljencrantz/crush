use crate::builtins::pty::session::PtySession;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::signature::patterns::Patterns;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use chrono::Duration;
use ordered_map::OrderedMap;
use signature::signature;
use std::sync::OnceLock;

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        Write::declare_method(&mut res);
        WaitFor::declare_method(&mut res);
        Contents::declare_method(&mut res);
        Pid::declare_method(&mut res);
        Alive::declare_method(&mut res);
        Close::declare_method(&mut res);
        res
    })
}

#[signature(
    pty.session.write,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Write literal text to this virtual terminal, as if typed at the keyboard.",
    long = "No newline or key translation is added -- include `\\n` explicitly to submit a",
    long = "command line, and use `$t:control:<name>` (see `pty:control`) for control",
    long = "characters such as Ctrl-Z.",
    example = "$t:write \"echo hello\\n\"",
    example = "$t:write $t:control:z",
)]
struct Write {
    #[description("the literal text to send.")]
    text: String,
}

fn write(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Write = Write::parse(context.remove_arguments(), &context.global_state.printer())?;
    let session = PtySession::get(&context.this.r#struct()?)?;
    session.write_raw(&cfg.text)?;
    context.output.send(Value::Empty)
}

#[signature(
    pty.session.wait_for,
    can_block = true,
    output = Known(ValueType::Empty),
    short = "Block until the virtual terminal's rendered screen contains a pattern, or error out after a timeout.",
    long = "`pattern` may be a plain string (substring match), a glob, or a regular expression.",
    long = "",
    long = "A glob's `*` does not match `/`, and the screen almost always contains one (e.g.",
    long = "the prompt's current directory), so a glob like `*hello*` will usually never match.",
    long = "Use a plain string, or a regular expression such as `^(hel+o)`, for anything that",
    long = "may need to match across `/` or arbitrary screen content.",
    example = "$t:wait_for \"# \"",
    example = "$t:wait_for ^(hel+o)",
)]
struct WaitFor {
    #[description("the text (string, glob, or regular expression) to wait for. A glob's `*` does not match `/`; prefer a regular expression.")]
    pattern: Patterns,
    #[default(Duration::seconds(5))]
    #[description("how long to wait before giving up.")]
    timeout: Duration,
}

fn wait_for(mut context: CommandContext) -> CrushResult<()> {
    let cfg: WaitFor =
        WaitFor::parse(context.remove_arguments(), &context.global_state.printer())?;
    let session = PtySession::get(&context.this.r#struct()?)?;
    session.wait_for(&cfg.pattern, cfg.timeout)?;
    context.output.send(Value::Empty)
}

#[signature(
    pty.session.contents,
    can_block = false,
    output = Known(ValueType::String),
    short = "The virtual terminal's current rendered screen, as plain text.",
)]
struct Contents {}

fn contents(mut context: CommandContext) -> CrushResult<()> {
    let session = PtySession::get(&context.this.r#struct()?)?;
    context.output.send(Value::from(session.contents()))
}

#[signature(
    pty.session.pid,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "The process id of the command this virtual terminal is running.",
)]
struct Pid {}

fn pid(mut context: CommandContext) -> CrushResult<()> {
    let session = PtySession::get(&context.this.r#struct()?)?;
    context.output.send(Value::from(session.pid()))
}

#[signature(
    pty.session.alive,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "Whether the command this virtual terminal is running is still alive.",
)]
struct Alive {}

fn alive(mut context: CommandContext) -> CrushResult<()> {
    let session = PtySession::get(&context.this.r#struct()?)?;
    context.output.send(Value::Bool(session.alive()?))
}

#[signature(
    pty.session.close,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Terminate this virtual terminal's command and release its resources.",
)]
struct Close {}

fn close(mut context: CommandContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    match this.get("id") {
        Some(Value::Integer(id)) => {
            PtySession::close(id as i32);
            context.output.send(Value::Empty)
        }
        _ => crate::lang::errors::command_error("Invalid method id"),
    }
}
