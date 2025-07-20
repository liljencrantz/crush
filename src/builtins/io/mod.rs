use crate::data::table::ColumnFormat;
use crate::lang::command::OutputType::Known;
use crate::lang::data::list::List;
use crate::lang::errors::{CrushResult, data_error};
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

pub fn val(context: CommandContext) -> CrushResult<()> {
    let cfg: Val = Val::parse(context.arguments, &context.global_state.printer())?;
    context.output.send(cfg.value)
}

#[signature(
    io.dir,
    can_block = false,
    short = "List members of value",
    output = Known(ValueType::List(Box::from(ValueType::String))),
    example = "dir .")]
struct Dir {
    #[description("the value to list the members of.")]
    value: Value,
}

pub fn dir(context: CommandContext) -> CrushResult<()> {
    let cfg: Dir = Dir::parse(context.arguments, &context.global_state.printer())?;
    context.output.send(
        List::new(
            ValueType::String,
            cfg.value
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
    short = "Prints all arguments directly to the screen",
    long = "This command may at first appear pointless, since values entered to the prompt are printed to the screen by default. But inside of code blocks, results are either completely ignored or returned as the return value of the block. In these situations, the `echo` command is useful for passing values to the user.",
    example = "echo \"Hello, world!\""
)]
struct Echo {
    #[description("the values to print.")]
    #[unnamed()]
    values: Vec<Value>,
    #[description("do not escape control characters in string values.")]
    #[default(false)]
    raw: bool,
}

fn echo(context: CommandContext) -> CrushResult<()> {
    let cfg: Echo = Echo::parse(context.arguments, &context.global_state.printer())?;
    let pretty = PrettyPrinter::new(
        context.global_state.printer().clone(),
        context.global_state.format_data(),
    );
    for value in cfg.values {
        match (cfg.raw, &value) {
            (true, Value::String(s)) => context.global_state.printer().line(s),

            _ => pretty.print_value(value, &ColumnFormat::None),
        }
    }
    context.output.empty()
}

#[signature(
    io.member,
    can_block = false,
    short = "Extract one member from the input struct.",
    example = "$uri := \"https://raw.githubusercontent.com/liljencrantz/crush/refs/heads/master/example_data/dinosaurs.json\"",
    example = "http $uri | member body | json:from"
)]
struct Member {
    #[description("the member to extract.")]
    field: String,
}

fn member(context: CommandContext) -> CrushResult<()> {
    let cfg: Member = Member::parse(context.arguments, &context.global_state.printer())?;
    match context.input.recv()? {
        Value::Struct(s) => context.output.send(
            s.get(&cfg.field)
                .ok_or(format!("Unknown field \"{}\"", cfg.field).as_str())?,
        ),
        _ => data_error("`member`: Expected a struct"),
    }
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

fn readline(context: CommandContext) -> CrushResult<()> {
    let cfg: Readline = Readline::parse(context.arguments, &context.global_state.printer())?;

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
                .line(&format!("`readline`: Failed to save history: {}", err))
        }
    }

    context.output.send(Value::from(line))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "io",
        "Data serialization I/O",
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

            http::Http::declare(env)?;
            Echo::declare(env)?;
            Member::declare(env)?;
            Val::declare(env)?;
            Dir::declare(env)?;
            Readline::declare(env)?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
