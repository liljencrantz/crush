use crate::commands::CompileContext;
use crate::errors::{CrushResult, error};
use crate::env::Env;
use crate::data::{Value, Command, Argument, RowsReader};
use crate::stream::{Readable, ValueSender};

fn parse(arguments: Vec<Argument>) -> CrushResult<Vec<usize>> {
    Ok(vec![0,1])
}

fn run(
    columns: Vec<usize>,
    mut input: impl Readable,
    sender: ValueSender,
) -> CrushResult<()> {
    Ok(())
}

fn add(context: CompileContext) -> CrushResult<()> {
    let columns = parse(context.arguments)?;
    match context.input.recv()? {
        Value::Stream(s) => run(columns, s.stream, context.output),
        Value::Rows(r) => run(columns, RowsReader::new(r), context.output),
        _ => error("Expected a stream"),
    }
}

pub fn declare(root: &Env) -> CrushResult<()> {
    let list = root.create_namespace("smath")?;
    list.declare_str("add", Value::Command(Command::new(add)))?;
    Ok(())
}
