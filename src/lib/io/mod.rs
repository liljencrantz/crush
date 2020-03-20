use crate::lang::scope::Scope;
use crate::lang::errors::CrushResult;
use crate::lang::{value::Value, command::SimpleCommand, command::ExecutionContext, binary::BinaryReader};
use crate::lang::stream_printer::print_value;
use crate::lib::parse_util::argument_files;

mod lines;
mod csv;
mod json;
mod http;

pub fn val(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.arguments.remove(0).value)
}

fn echo(mut context: ExecutionContext) -> CrushResult<()> {
    for arg in context.arguments.drain(..) {
        print_value(arg.value);
    }
    Ok(())
}

fn cat(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::BinaryStream(BinaryReader::paths(argument_files(context.arguments)?)?))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("io")?;
    root.r#use(&env);
    env.declare("cat", Value::Command(SimpleCommand::new(cat, true)))?;
    env.declare("http", Value::Command(SimpleCommand::new(http::perform, true)))?;
    env.declare("lines", Value::Command(SimpleCommand::new(lines::perform, true)))?;
    env.declare("csv", Value::Command(SimpleCommand::new(csv::perform, true)))?;
    env.declare("json", Value::Command(SimpleCommand::new(json::perform, true)))?;
    env.declare("echo", Value::Command(SimpleCommand::new(echo, false)))?;
    env.declare("val", Value::Command(SimpleCommand::new(val, false)))?;
    env.readonly();

    Ok(())
}
