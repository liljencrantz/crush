use crate::lang::command::OutputType::Known;
use crate::lang::errors::{argument_error_legacy, data_error, mandate, CrushResult};
use crate::lang::data::list::List;
use crate::lang::pretty::PrettyPrinter;
use crate::lang::data::scope::Scope;
use crate::lang::value::{Field, ValueType};
use crate::lang::{execution_context::CommandContext, value::Value};
use signature::signature;
use num_format::SystemLocale;

mod bin;
mod csv;
mod http;
mod json;
mod lines;
mod pup;
mod split;
mod toml;
mod words;
mod yaml;

#[signature(val,
can_block = false,
short = "Return value",
output = Known(ValueType::Any),
example = "val val",
long = "This command is useful if you want to pass a command as input in\n    a pipeline instead of executing it. It is different from the echo command\n    in that val sends the value through the pipeline, whereas echo prints it to screen.")]
struct Val {
    #[description("the value to pass as output.")]
    value: Value,
}

pub fn val(context: CommandContext) -> CrushResult<()> {
    let cfg: Val = Val::parse(context.arguments, &context.printer)?;
    context.output.send(cfg.value)
}

#[signature(dir,
can_block = false,
short = "List members of value",
output = Known(ValueType::List(Box::from(ValueType::String))),
example = "dir .")]
struct Dir {
    #[description("the value to list the members of.")]
    value: Value,
}

pub fn dir(context: CommandContext) -> CrushResult<()> {
    let cfg: Dir = Dir::parse(context.arguments, &context.printer)?;
    context.output.send(Value::List(List::new(
        ValueType::String,
        cfg.value
            .fields()
            .drain(..)
            .map(|n| Value::String(n))
            .collect(),
    )))
}

#[signature(echo, can_block = false, short = "Prints all arguments directly to the screen", output = Known(ValueType::Empty), example = "echo \"Hello, world!\"")]
struct Echo {
    #[description("the values to print.")]
    #[unnamed()]
    values: Vec<Value>,
}

fn echo(context: CommandContext) -> CrushResult<()> {
    let cfg: Echo = Echo::parse(context.arguments, &context.printer)?;
    let pretty = PrettyPrinter::new(context.printer.clone(), context.global_state.grouping());
    for value in cfg.values {
        pretty.print_value(value);
    }
    context.output.send(Value::Empty())
}

#[signature(
    member,
    can_block = false,
    short = "Extracts one member from the input struct.",
    example = "http \"example.com\" | member ^body | json:from"
)]
struct Member {
    #[description("the member to extract.")]
    field: Field,
}

fn member(context: CommandContext) -> CrushResult<()> {
    let cfg: Member = Member::parse(context.arguments, &context.printer)?;
    if cfg.field.len() != 1 {
        return argument_error_legacy("Invalid field - should have exactly one element");
    }
    match context.input.recv()? {
        Value::Struct(s) => context.output.send(mandate(
            s.get(&cfg.field[0]),
            format!("Unknown field \"{}\"", cfg.field[0]).as_str(),
        )?),
        _ => data_error("Expected a struct"),
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "io",
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

            http::Http::declare(env)?;
            Echo::declare(env)?;
            Member::declare(env)?;
            Val::declare(env)?;
            Dir::declare(env)?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
