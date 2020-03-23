use crate::lang::scope::Scope;
use crate::lang::errors::CrushResult;
use crate::lang::{value::Value, command::ExecutionContext, binary::BinaryReader};
use crate::lang::stream_printer::print_value;
use crate::lib::parse_util::argument_files;
use crate::lang::command::CrushCommand;
use crate::lang::list::List;
use crate::lang::value::ValueType;

mod lines;
mod csv;
mod json;
mod http;

pub fn val(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.arguments.remove(0).value)
}

pub fn dir(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(
        Value::List(List::new(ValueType::String,
                              context.arguments.remove(0).value.fields()
                                  .drain(..)
                                  .map(|n| Value::String(n))
                                  .collect()))
    )
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
    env.declare("cat", Value::Command(CrushCommand::command(cat, true)))?;
    env.declare("http", Value::Command(CrushCommand::command(http::perform, true)))?;
    env.declare("lines", Value::Command(CrushCommand::command(lines::perform, true)))?;
    env.declare("csv", Value::Command(CrushCommand::command(csv::perform, true)))?;
    env.declare("json", Value::Command(CrushCommand::command(json::perform, true)))?;
    env.declare("echo", Value::Command(CrushCommand::command(echo, false)))?;
    env.declare("val", Value::Command(CrushCommand::command(val, false)))?;
    env.declare("dir", Value::Command(CrushCommand::command(dir, false)))?;
    env.readonly();

    Ok(())
}
