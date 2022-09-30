use signature::signature;
use crate::lang::help::Help;
use crate::lang::value::Value;
use crate::{CrushResult, Printer};
use crate::lang::errors::error;
use crate::state::contexts::CommandContext;
use crate::lang::command::OutputType::Known;
use crate::lang::value::ValueType;

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

pub fn help(mut context: CommandContext) -> CrushResult<()> {
    let cfg: HelpSignature = HelpSignature::parse(context.remove_arguments(), &context.global_state.printer())?;
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
