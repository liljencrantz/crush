use crate::lang::command::CrushCommand;
use crate::lang::errors::{argument_error, error, CrushResult};
use crate::lang::execution_context::ExecutionContext;
use crate::lang::stream::{black_hole, channels, empty_channel, Readable};
use crate::lang::{argument::Argument, table::ColumnType};
use crate::{
    lang::stream::OutputStream,
    lang::{table::Row, value::Value},
};

fn evaluate(
    condition: Box<dyn CrushCommand + Send + Sync>,
    row: &Row,
    input_type: &[ColumnType],
    base_context: &ExecutionContext,
) -> CrushResult<bool> {
    let arguments = row
        .clone()
        .into_vec()
        .drain(..)
        .zip(input_type.iter())
        .map(|(c, t)| Argument::named(t.name.as_ref(), c))
        .collect();

    let (sender, reciever) = channels();

    condition.invoke(
        base_context
            .clone()
            .with_args(arguments, None)
            .with_sender(sender),
    )?;

    match reciever.recv()? {
        Value::Bool(b) => Ok(b),
        _ => error("Expected a boolean result"),
    }
}

pub fn run(
    condition: Box<dyn CrushCommand + Send + Sync>,
    input: &mut dyn Readable,
    output: OutputStream,
    base_context: &ExecutionContext,
) -> CrushResult<()> {
    while let Ok(row) = input.read() {
        match evaluate(condition.clone(), &row, input.types(), &base_context) {
            Ok(val) => {
                if val {
                    if output.send(row).is_err() {
                        break;
                    }
                }
            }
            Err(e) => base_context.printer.crush_error(e),
        }
    }
    Ok(())
}

pub fn parse(
    _input_type: &[ColumnType],
    arguments: &mut Vec<Argument>,
) -> CrushResult<Box<dyn CrushCommand + Send + Sync>> {
    match arguments.remove(0).value {
        Value::Command(c) => Ok(c),
        _ => argument_error("Expected a closure"),
    }
}

pub fn r#where(mut context: ExecutionContext) -> CrushResult<()> {
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
            run(
                parse(input.types(), context.arguments.as_mut())?,
                input.as_mut(),
                output,
                &base_context,
            )
        }
        None => error("Expected a stream"),
    }
}
