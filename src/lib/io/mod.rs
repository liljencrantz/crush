use crate::lang::scope::Scope;
use crate::lang::errors::CrushResult;
use crate::lang::{value::Value, execution_context::ExecutionContext, execution_context::ArgumentVector};
use crate::lang::list::List;
use crate::lang::value::ValueType;
use crate::lang::pretty_printer::PrettyPrinter;
use crate::lang::argument::ArgumentHandler;
use crate::lang::command::OutputType::{Known};

mod bin;
mod csv;
mod http;
mod json;
mod lines;
mod pup;
mod toml;

pub fn val(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context.output.send(context.arguments.value(0)?)
}

pub fn dir(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context.output.send(
        Value::List(List::new(
            ValueType::String,
            context.arguments.value(0)?.fields()
                .drain(..)
                .map(|n| Value::String(n))
                .collect()))
    )
}

fn echo(mut context: ExecutionContext) -> CrushResult<()> {
    for arg in context.arguments.drain(..) {
        PrettyPrinter::new(context.printer.clone()).print_value(arg.value);
    }
    context.output.send(Value::Empty())
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
            http::Http::declare(env)?;

            env.declare_command(
                "echo", echo, false,
                "echo @value:any", "Prints all arguments directly to the screen", None, Known(ValueType::Empty))?;
            env.declare_command(
                "val", val, false,
                "val value:any",
                "Return value",
                Some(r#"    This command is useful if you want to e.g. pass a command as input in
    a pipeline instead of executing it. It is different from the echo command
    in that val sends the value thorugh the pipeline, where echo prints it to screen."#), Known(ValueType::Any))?;
            env.declare_command(
                "dir", dir, false,
                "dir value:any", "List members of value", None, Known(ValueType::Empty))?;
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
