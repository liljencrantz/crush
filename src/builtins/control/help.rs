use crate::CrushResult;
use crate::lang::command::OutputType::Known;
use crate::lang::help::Help;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::state::contexts::CommandContext;
use crate::util::highlight::highlight_colors;
use signature::signature;
use std::collections::HashSet;

#[signature(
    control.help,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Show help on the specified value.",
    long = "The help command will show you help about a thing that you pass in. If you,",
    long = "for example pass in an integer (e.g. `help 3`), then you will see a help",
    long = "message about how crush represents integers and what methods an integer",
    long = "holds. You can also pass in any command to help (e.g. `help $files` for help",
    long = "on the `files` command). Note that you will need to prepend the `$` sigil to",
    long = "the command name, since you're not using it as the command name.",
    long = "",
    long = "If `help`'s input is a pipeline, `help` shows help on the value in the",
    long = "pipeline. Otherwise, if a topic argument is given, `help` shows help on that",
    long = "value instead. With neither, `help` shows this introductory message.",
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

    // Unlike a plain `recv_or` (dir/member/typeof/convert), no topic at all isn't an
    // error here -- it's what shows the welcome message below -- so a connected pipe
    // is only preferred over the topic argument, never required the way it is there.
    let topic: Option<Value> = if context.input.is_pipeline() {
        Some(context.input.recv()?)
    } else {
        cfg.topic
    };

    let map = highlight_colors(&context.scope);

    // The "accepts the following arguments" list this signature macro
    // generates backtick-wraps each argument's name and its default/
    // allowed values purely for visual styling (see render_html's doc
    // comment) -- collected here, before topic is consumed below, so
    // format=html can tell those apart from a real cross-reference. A
    // type's own member list (ValueType::long_help_methods, e.g. float's
    // `min`/`max`/`is_nan`) is generated the same way and has the exact
    // same problem -- "min" could just as easily name a real, unrelated
    // command (stream:min).
    let own_names: HashSet<String> = match &topic {
        Some(Value::Command(cmd)) => cmd
            .completion_data()
            .iter()
            .flat_map(|p| {
                let mut names = vec![p.name.clone()];
                if let Some(default) = &p.default {
                    names.push(default.to_string());
                }
                if let Some(allowed) = &p.allowed {
                    names.extend(allowed.iter().map(|v| v.to_string()));
                }
                names
            })
            .collect(),
        Some(Value::Type(t)) => t.fields().into_iter().map(|(k, _)| k.clone()).collect(),
        _ => HashSet::new(),
    };

    let s = match topic {
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
        "html" => context.output.send(Value::from(crate::util::md::render_html(
            s,
            &own_names,
        )?)),
        "terminal" => {
            context
                .global_state
                .printer()
                .line(&crate::util::md::render(
                    s,
                    context.global_state.printer().width(),
                    map,
                )?);
            context.output.send(Value::Empty)
        }
        _ => unreachable!(),
    }
}
