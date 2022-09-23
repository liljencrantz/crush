use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, error, to_crush_error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::help::Help;
use crate::lang::printer::Printer;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::file::{cwd, home};
use signature::signature;
use crate::lang::signature::files::Files;
use std::path::PathBuf;
use std::convert::TryFrom;

mod usage;
mod files;
pub mod fd;

#[signature(
cd,
can_block=false,
output = Known(ValueType::Empty),
short = "Change to the specified working directory.",
)]
struct Cd {
    #[unnamed()]
    #[description("the new working directory.")]
    destination: Files,
}

fn cd(context: CommandContext) -> CrushResult<()> {
    let cfg: Cd = Cd::parse(context.arguments, &context.global_state.printer())?;

    let dir = match cfg.destination.had_entries() {
        true => PathBuf::try_from(cfg.destination),
        false => home(),
    }?;

    to_crush_error(std::env::set_current_dir(dir))?;
    context.output.send(Value::Empty)
}

#[signature(
pwd,
can_block=false,
output = Known(ValueType::File),
short = "Return the current working directory.",
)]
struct Pwd {}

fn pwd(context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::from(cwd()?))
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
example = "help $ls",
example = "help $integer",
example = "help $help",
)]
pub struct HelpSignature {
    #[description("the topic you want help on.")]
    topic: Option<Value>,
}

pub fn help(context: CommandContext) -> CrushResult<()> {
    let cfg: HelpSignature = HelpSignature::parse(context.arguments, &context.global_state.printer())?;
    match cfg.topic {
        None => {
            context.global_state.printer().line(
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
            context.output.send(Value::Empty)
        }
        Some(v) => {
            match v {
                Value::String(f) => match &context.scope.get_calling_scope()?.get(&f)? {
                    None => error(format!("Unknown identifier {}", &f))?,
                    Some(v) => halp(v, &context.global_state.printer()),
                },
                Value::Command(cmd) => halp(cmd.help(), &context.global_state.printer()),
                Value::Type(t) => halp(&t, &context.global_state.printer()),
                v => halp(&v, &context.global_state.printer()),
            }
            context.output.send(Value::Empty)
        }
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "fs",
        "File system introspection",
        Box::new(move |fs| {
            files::FilesSignature::declare(fs)?;
            Cd::declare(fs)?;
            Pwd::declare(fs)?;
            HelpSignature::declare(fs)?;
            usage::Usage::declare(fs)?;
            fs.create_namespace(
                "fd",
                "Information on currently open files and sockets",
                Box::new(move |fd| {
                    fd::File::declare(fd)?;
                    #[cfg(target_os = "linux")]
                    fd::procfs::Network::declare(fd)?;
                    #[cfg(target_os = "linux")]
                    fd::procfs::Unix::declare(fd)?;
                    Ok(())
                }),
            )?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
