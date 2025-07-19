use crate::CrushResult;
use crate::lang::command::OutputType::Known;
use crate::lang::help::Help;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::state::contexts::CommandContext;
use crate::util::highlight::highlight_colors;
use signature::signature;

#[signature(
    control.help,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Show help on the specified value.",
    long = "The help command will show you help about a thing that you pass in. If you, for example pass in an integer (e.g. `help 3`), then you will see a help message about how crush represents integers and what methods an integer holds. You can also pass in any command to help (e.g. `help $files` for help on the `files` command). Note that you will need to prepend the `$` sigil to the command name, since you're not using it as the command name.",
    example = "# Show this message",
    example = "help $help",
    example = "# Show help on the root namespace",
    example = "help $global",
)]
pub struct HelpSignature {
    #[description("the topic you want help on.")]
    topic: Option<Value>,
    #[default("terminal")]
    #[description(
        "output format. The default, `terminal`, will render the help directly into the terminal. The other formats return a string containing either an html fragment or markdown."
    )]
    #[values("html", "markdown", "terminal")]
    format: String,
}


pub fn help(mut context: CommandContext) -> CrushResult<()> {
    let cfg: HelpSignature =
        HelpSignature::parse(context.remove_arguments(), &context.global_state.printer())?;

    let map = highlight_colors(&context.scope);
    
    let s = match cfg.topic {
        None => {
            r#"
# Welcome to Crush!

If this is your first time using Crush, congratulations on just entering your
first command! If you haven't already, you might want to check out the Readme
for an introduction at https://github.com/liljencrantz/crush/.

Call the help command with the name of any value, including a command or a
type in order to get help about it. For example, you might want to run the
commands `help $help`, `help $string`, `help $if` or `help $where`.

To get a list of everything in your namespace, write `var:list`. To list the
members of a value, write `dir <value>`.
"#
        }
        Some(o) => match o.long_help() {
            None => &format!("    {}\n\n{}", o.signature(), o.short_help()),
            Some(long_help) => &format!(
                "    {}\n\n{}\n\n{}",
                o.signature(),
                o.short_help(),
                long_help
            ),
        },
    };

    match cfg.format.as_str() {
        "markdown" => context.output.send(Value::from(s)),
        "html" => context.output.send(Value::from(markdown::to_html(s))),
        "terminal" => {
            context.global_state.printer().line(&crate::util::md::render(
                s,
                context.global_state.printer().width(),
                map,
            )?);
            context.output.send(Value::Empty)
        }
        _ => unreachable!(),
    }
}
