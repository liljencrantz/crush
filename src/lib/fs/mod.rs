use crate::lang::command::OutputType::Known;
use crate::lang::errors::{argument_error, to_crush_error, CrushResult};
use crate::lang::execution_context::ArgumentVector;
use crate::lang::execution_context::CommandContext;
use crate::lang::help::Help;
use crate::lang::printer::Printer;
use crate::lang::data::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::file::{cwd, home};
use signature::signature;
use crate::lang::files::Files;

mod find;

#[signature(
cd,
can_block=false,
output = Known(ValueType::Empty),
short = "Change to the specified working directory.",
)]
pub struct Cd {
    #[unnamed()]
    #[description("the new working directory.")]
    destination: Files,
}

pub fn cd(context: CommandContext) -> CrushResult<()> {
    let cfg: Cd = Cd::parse(context.arguments, &context.printer)?;

    let dir = match cfg.destination.had_entries() {
        true => cfg.destination.into_file(),
        false => home(),
    }?;

    to_crush_error(std::env::set_current_dir(dir))?;
    context.output.send(Value::Empty())
}

#[signature(
pwd,
can_block=false,
output = Known(ValueType::File),
short = "Return the current working directory.",
)]
pub struct Pwd {}

pub fn pwd(context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::File(cwd()?))
}

fn halp(o: &dyn Help, printer: &Printer) {
    printer.line(
        match o.long_help() {
            None => format!("{}\n\n    {}", o.signature(), o.short_help()),
            Some(long_help) => format!(
                "{}\n\n    {}\n\n{}",
                o.signature(),
                o.short_help(),
                long_help
            ),
        }
        .as_str(),
    );
}

#[signature(
help,
can_block=false,
output = Known(ValueType::Empty),
short = "Show help about the specified thing.",
example = "help ls",
example = "help integer",
example = "help help",
)]
pub struct HelpSignature {
    #[description("the topic you want help on.")]
    topic: Option<Value>,
}

pub fn help(mut context: CommandContext) -> CrushResult<()> {
    let cfg: HelpSignature = HelpSignature::parse(context.arguments, &context.printer)?;
    match cfg.topic {
        None => {
            context.printer.line(
                r#"
Welcome to Crush!

If this is your first time using Crush, congratulations on just entering your
first command! If you haven't already, you might want to check out the Readme
for an introduction at https://github.com/liljencrantz/crush/.

Call the help command with the name of any value, including a command or a
type in order to get help about it. For example, you might want to run the
commands "help help", "help string", "help if" or "help where".

To get a list of everything in your namespace, write "var:env". To list the
members of a value, write "dir <value>".
"#,
            );
            context.output.send(Value::Empty())
        }
        Some(v) => {
            match v {
                Value::Command(cmd) => halp(cmd.help(), &context.printer),
                Value::Type(t) => halp(&t, &context.printer),
                v => halp(&v, &context.printer),
            }
            context.output.send(Value::Empty())
        }
        _ => argument_error("The help command expects at most one argument"),
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "fs",
        Box::new(move |env| {
            find::Find::declare(env)?;
            Cd::declare(env)?;
            Pwd::declare(env)?;
            HelpSignature::declare(env)?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
