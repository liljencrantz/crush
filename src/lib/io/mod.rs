use crate::lang::argument::ArgumentHandler;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{argument_error, data_error, mandate, CrushResult};
use crate::lang::list::List;
use crate::lang::pretty_printer::PrettyPrinter;
use crate::lang::scope::Scope;
use crate::lang::value::{Field, ValueType};
use crate::lang::{
    execution_context::ArgumentVector, execution_context::ExecutionContext, value::Value,
};
use signature::signature;

mod bin;
mod csv;
mod http;
mod json;
mod lines;
mod pup;
mod split;
mod toml;
mod words;

pub fn val(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context.output.send(context.arguments.value(0)?)
}

pub fn dir(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context.output.send(Value::List(List::new(
        ValueType::String,
        context
            .arguments
            .value(0)?
            .fields()
            .drain(..)
            .map(|n| Value::String(n))
            .collect(),
    )))
}

#[signature(echo, can_block=false, short="Prints all arguments directly to the screen", output = Known(ValueType::Empty), example="echo \"Hello, world!\"")]
struct Echo {
    #[description("the values to print.")]
    #[unnamed()]
    values: Vec<Value>,
}

fn echo(context: ExecutionContext) -> CrushResult<()> {
    let cfg: Echo = Echo::parse(context.arguments, &context.printer)?;
    let pretty = PrettyPrinter::new(context.printer.clone());
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

fn member(context: ExecutionContext) -> CrushResult<()> {
    let cfg: Member = Member::parse(context.arguments, &context.printer)?;
    if cfg.field.len() != 1 {
        return argument_error("Invalid field - should have exactly one element");
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
    let e = root.create_lazy_namespace(
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

            http::Http::declare(env)?;
            Echo::declare(env)?;
            Member::declare(env)?;
            env.declare_command(
                "val",
                val,
                false,
                "val value:any",
                "Return value",
                Some(
                    r#"    This command is useful if you want to e.g. pass a command as input in
    a pipeline instead of executing it. It is different from the echo command
    in that val sends the value thorugh the pipeline, where echo prints it to screen."#,
                ),
                Known(ValueType::Any),
            )?;
            env.declare_command(
                "dir",
                dir,
                false,
                "dir value:any",
                "List members of value",
                None,
                Known(ValueType::Empty),
            )?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
