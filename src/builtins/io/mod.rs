use crate::data::table::ColumnFormat;
use crate::lang::command::OutputType::Known;
use crate::lang::data::list::List;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::errors::CrushResult;
use crate::lang::interactive::config_dir;
use crate::lang::pretty::PrettyPrinter;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use rustyline::Editor;
use rustyline::history::DefaultHistory;
use signature::signature;
use std::path::PathBuf;

mod base64;
mod bin;
mod csv;
mod hex;
mod http;
pub mod json;
mod lines;
mod percent;
mod pup;
mod split;
mod toml;
mod words;
mod yaml;

#[signature(
    io.val,
    can_block = false,
    short = "Return value",
    output = Known(ValueType::Any),
    long = "This command is useful if you want to pass a command as input in a pipeline instead of executing it. It is different from the echo command in that val sends the value through the pipeline, whereas echo prints it to screen.",
    example = "val $val",
)]
struct Val {
    #[description("the value to pass as output.")]
    value: Value,
}

pub fn val(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Val = Val::parse(context.remove_arguments(), &context.global_state.printer())?;
    let res = context.output.send(cfg.value);
    res
}

#[signature(
    io.dir,
    can_block = false,
    short = "List the member names of a value.",
    output = Known(ValueType::List(Box::from(ValueType::String))),
    long = "Works on any value -- a struct's own fields, a scope's local variables, or",
    long = "(for anything else, including a type value like `$float`) the methods its",
    long = "type declares. If `dir`'s input is a pipeline, `dir` lists the members of the",
    long = "value in the pipeline. Otherwise, `dir` requires a value to be provided as an",
    long = "argument and lists the members of that value.",
    long = "",
    long = "Pair with `member` to fetch one of the listed names -- `member`'s own name",
    long = "argument, unlike the `:` operator, can be a runtime value instead of a fixed",
    long = "word in the source, so the two together let you enumerate and read members",
    long = "whose names aren't known ahead of time.",
    example = "dir .",
    example = "# The full help text of every method float has",
    example = "dir $float | each {|$name| help ($float | member $name)}",
)]
struct Dir {
    #[description("the value to list the members of.")]
    value: Option<Value>,
}

pub fn dir(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Dir = Dir::parse(context.remove_arguments(), &context.global_state.printer())?;
    let value = match cfg.value {
        Some(value) => value,
        None => context.input.recv()?,
    };
    context.output.send(
        List::new(
            ValueType::String,
            value
                .fields()
                .drain(..)
                .map(|n| Value::from(n))
                .collect::<Vec<_>>(),
        )
        .into(),
    )
}

#[signature(
    io.echo,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Prints all arguments directly to standard output.",
    long = "If no arguments are passed to the `values` parameter, print the input pipeline value instead.",
    long = "",
    long = "This command may at first appear pointless, since values entered on the prompt are printed to standard output by default. But that is only true when running a command interactively. When executing a file or running a closure, results are either completely ignored or returned as the return value of the block. In these situations, the `echo` command is useful for making sure a given value is written to standard output.",
    example = "# These command invocations are equivalent",
    example = "echo \"Hello, world!\"",
    example = "\"Hello, world!\" | echo",
)]
struct Echo {
    #[description("the values to print.")]
    #[unnamed()]
    values: Vec<Value>,
    #[description("do not escape control characters in string values.")]
    #[default(false)]
    raw: bool,
}

fn echo(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Echo = Echo::parse(context.remove_arguments(), &context.global_state.printer())?;
    let pretty = PrettyPrinter::new_tracked(
        context.global_state.printer().clone(),
        context.global_state.format_data(),
        context.global_state.threads().clone(),
        context.command_handle().clone(),
    );
    if cfg.values.is_empty() {
        pretty.print_value(context.input.recv()?, &ColumnFormat::None);
    } else {
        for value in cfg.values {
            match (cfg.raw, &value) {
                (true, Value::String(s)) => context.global_state.printer().line(s),

                _ => pretty.print_value(value, &ColumnFormat::None),
            }
        }
    }
    context.output.empty()
}

#[signature(
    io.member,
    can_block = false,
    short = "Extract one named member from the input value.",
    long = "Works like the `:` member operator, except the member name is a runtime",
    long = "value (e.g. a variable) rather than a fixed word in the source -- use this",
    long = "when the name to look up isn't known until the script runs. Pair with `dir`",
    long = "to discover a value's member names first.",
    example = "$uri := \"https://raw.githubusercontent.com/liljencrantz/crush/refs/heads/master/example_data/dinosaurs.json\"",
    example = "http $uri | member body | json:from",
    example = "# dir lists a value's member names; member fetches one by name -- together",
    example = "# they let you enumerate members whose names aren't known ahead of time",
    example = "for name=$(dir 5) { echo (5 | member $name) }",
)]
struct Member {
    #[description("the member to extract.")]
    field: String,
}

fn member(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Member = Member::parse(context.remove_arguments(), &context.global_state.printer())?;
    let value = context.input.recv()?;
    let result = value.field(&cfg.field)?.ok_or_else(|| {
        format!(
            "Missing field `{}` in value of type `{}`",
            cfg.field,
            value.value_type()
        )
    })?;
    context.output.send(result)
}

static MEMBERS_OUTPUT_TYPE: [ColumnType; 2] = [
    ColumnType::new("name", ValueType::String),
    ColumnType::new("type", ValueType::Type),
];

#[signature(
    io.members,
    can_block = true,
    output = Known(ValueType::table_input_stream(&MEMBERS_OUTPUT_TYPE)),
    short = "List the columns of any streamable input value as name/type pairs.",
    long = "Works on anything that can be read as a stream of rows -- a table, a list, a",
    long = "dict, a struct, a scope -- and reports that stream's shape without consuming",
    long = "any of its actual rows.",
    example = "$my_dict | members",
)]
struct Members {}

fn members(mut context: CommandContext) -> CrushResult<()> {
    Members::parse(context.remove_arguments(), &context.global_state.printer())?;
    let input = context.input_stream()?;
    let output = context.initialize_output(&MEMBERS_OUTPUT_TYPE)?;
    for ct in input.types() {
        output.send(Row::new(vec![
            Value::from(ct.name().to_string()),
            Value::Type(ct.cell_type.clone()),
        ]))?;
    }
    Ok(())
}

fn history_file(name: &str) -> CrushResult<PathBuf> {
    Ok(config_dir()?.join(&format!("{}_history", name)))
}

#[signature(
    io.readline,
    short = "Read a string of input from the user.",
    long = "The readline command uses the same keyboard shortcuts as crush itself uses internally.",
    example = "# Ask the user for their name",
    example = "echo \"What is your name?\"",
    example = "$name := $(readline prompt=\"name: \")",
    example = "echo $(\"Hello, {}!\":format $name)",
    output = Known(ValueType::String),
)]
struct Readline {
    #[description("the prompt to show the user.")]
    #[default("crush# ")]
    prompt: String,

    #[description("load and save history under specified name.")]
    history: Option<String>,
}

fn readline(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Readline =
        Readline::parse(context.remove_arguments(), &context.global_state.printer())?;

    let mut rl = Editor::<(), DefaultHistory>::new()?;

    if let Some(history) = &cfg.history {
        let _ = rl.load_history(&history_file(&history)?);
    }

    let line = rl.readline(&cfg.prompt)?;

    if let Some(history) = &cfg.history {
        let _ = rl.add_history_entry(line.as_str());
        if let Err(err) = rl.save_history(&history_file(&history)?) {
            context
                .global_state
                .printer()
                .line(&format!("Failed to save history: {}", err))
        }
    }

    context.output.send(Value::from(line))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "io",
        "Data serialization I/O",
        Some(
            "Reading and writing structured data in specific wire formats -- `json`, `yaml`, \
             `toml`, `csv`, `hex`, `base64`, and `percent` encoding, and Crush's own native \
             `pup` format, each its own `to`/`from` pair. Also home to a few general-purpose \
             I/O commands that don't belong to any one format: `http`, `echo`, `readline`, \
             and `member`/`members` for pulling data out of a struct or stream. Imported into \
             the global scope, so e.g. `io:json:from` and bare `json:from` are the same \
             command."
                .to_string(),
        ),
        Box::new(move |env| {
            bin::declare(env)?;
            csv::declare(env)?;
            pup::declare(env)?;
            toml::declare(env)?;
            json::declare(env)?;
            lines::declare(env)?;
            split::declare(env)?;
            words::declare(env)?;
            yaml::declare(env)?;
            hex::declare(env)?;
            base64::declare(env)?;
            percent::declare(env)?;

            http::Http::declare(env)?;
            Echo::declare(env)?;
            Member::declare(env)?;
            Members::declare(env)?;
            Val::declare(env)?;
            Dir::declare(env)?;
            Readline::declare(env)?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
