use crate::data::table::ColumnFormat;
use crate::lang::command::OutputType::Known;
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
    example = "echo $(\"Hello, {}!\".format $name)",
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
             I/O commands that don't belong to any one format: `http`, `echo`, and \
             `readline`. Imported into the global scope, so e.g. `$io.json.from` and bare \
             `$json.from` are the same command."
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
            Readline::declare(env)?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
