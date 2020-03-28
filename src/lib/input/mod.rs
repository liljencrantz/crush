use crate::lang::scope::Scope;
use crate::lang::errors::CrushResult;
use crate::lang::{value::Value, execution_context::ExecutionContext, execution_context::ArgumentVector, binary::BinaryReader};
use crate::lang::pretty_printer::print_value;
use crate::lang::command::CrushCommand;
use crate::lang::list::List;
use crate::lang::value::ValueType;

mod lines;
mod csv;
mod json;
mod http;

pub fn val(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context.output.send(context.arguments.value(0)?)
}

pub fn dir(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context.output.send(
        Value::List(List::new(ValueType::String,
                              context.arguments.value(0)?.fields()
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

fn cat(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::BinaryStream(BinaryReader::paths(context.arguments.files()?)?))
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
