use crate::{
    lang::{
        value::Value,
        table::Row,
    },
    lang::stream::OutputStream,
};
use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::{error, CrushResult, argument_error};
use crate::lang::stream::{Readable, empty_channel, channels, black_hole};
use crate::lang::{table::ColumnType, argument::Argument};
use crate::lang::command::CrushCommand;

fn evaluate(
    condition: Box<dyn CrushCommand + Send + Sync>,
    row: &Row,
    input_type: &[ColumnType],
    base_context: &ExecutionContext) -> CrushResult<bool> {
    let arguments = row.clone().into_vec()
        .drain(..)
        .zip(input_type.iter())
        .map(|(c, t)| Argument::named(t.name.as_ref(), c))
        .collect();

    let (sender, reciever) = channels();

    condition.invoke(base_context.clone().with_args(arguments, None).with_sender(sender))?;

    match reciever.recv()? {
        Value::Bool(b) => Ok(b),
        _ => error("Expected a boolean result")
    }
}

pub fn run(condition: Box<dyn CrushCommand + Send + Sync>, input: &mut dyn Readable, output: OutputStream, base_context: &ExecutionContext) -> CrushResult<()> {
    while let Ok(row) = input.read() {
        match evaluate(condition.clone(), &row, input.types(), &base_context) {
            Ok(val) => if val { if output.send(row).is_err() { break; } },
            Err(e) => base_context.printer.crush_error(e),
        }
    }
    Ok(())
}

pub fn parse(_input_type: &[ColumnType],
             arguments: &mut Vec<Argument>) -> CrushResult<Box<dyn CrushCommand + Send + Sync>> {
    match arguments.remove(0).value {
        Value::Command(c) => Ok(c),
        _ => argument_error("Expected a closure"),
    }
}

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(mut input) => {
            let base_context = ExecutionContext {
                input: empty_channel(),
                output: black_hole(),
                arguments: vec![],
                env: context.env.clone(),
                this: None,
                printer: context.printer.clone(),
            };
            let output = context.output.initialize(input.types().to_vec())?;
            run(parse(input.types(), context.arguments.as_mut())?,
                input.as_mut(),
                output,
                &base_context)
        }
        None => error("Expected a stream"),
    }
}
