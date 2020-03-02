use crate::scope::Scope;
use crate::errors::CrushResult;
use crate::lang::{Value, SimpleCommand, ExecutionContext, BinaryReader};
use crate::stream_printer::print_value;
use crate::lib::parse_util::argument_files;

mod lines;
mod csv;
mod json;
mod http;

fn val(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.arguments.remove(0).value)
}

fn echo(mut context: ExecutionContext) -> CrushResult<()> {
    for arg in context.arguments.drain(..) {
        print_value(&context.printer, arg.value);
    }
    Ok(())
}

fn cat(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::BinaryStream(BinaryReader::paths(argument_files(context.arguments)?)?))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("io")?;
    root.uses(&env);
    env.declare_str("cat", Value::Command(SimpleCommand::new(cat)))?;
    env.declare_str("http", Value::Command(SimpleCommand::new(http::perform)))?;
    env.declare_str("lines", Value::Command(SimpleCommand::new(lines::perform)))?;
    env.declare_str("csv", Value::Command(SimpleCommand::new(csv::perform)))?;
    env.declare_str("json", Value::Command(SimpleCommand::new(json::perform)))?;
    env.declare_str("echo", Value::Command(SimpleCommand::new(echo)))?;
    env.declare_str("val", Value::Command(SimpleCommand::new(val)))?;
    env.readonly();

    Ok(())
}
