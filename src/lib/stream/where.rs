use crate::{
    lang::{
        value::Value,
        table::Row,
    },
    lang::stream::OutputStream,
};
use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::{error, CrushResult, argument_error};
use crate::lang::stream::{Readable, empty_channel, channels};
use crate::lang::{table::ColumnType, argument::Argument};
use crate::lang::scope::Scope;
use crate::lang::command::CrushCommand;
use crate::lang::printer::Printer;

fn evaluate(condition: Box<dyn CrushCommand + Send + Sync>, row: &Row, input_type: &Vec<ColumnType>, env: &Scope, printer: &Printer) -> CrushResult<bool> {
    let arguments = row.clone().into_vec()
        .drain(..)
        .zip(input_type.iter())
        .map(|(c, t)| Argument::named(t.name.as_ref(), c))
        .collect();

    let (sender, reciever) = channels();

    condition.invoke(ExecutionContext {
        input: empty_channel(),
        output: sender,
        arguments,
        env: env.clone(),
        this: None,
        printer: printer.clone(),
    })?;

    match reciever.recv()? {
        Value::Bool(b) => Ok(b),
        _ => error("Expected a boolean result")
    }
}

pub fn run(condition: Box<dyn CrushCommand + Send + Sync>, input: &mut dyn Readable, output: OutputStream, env: Scope, printer: &Printer) -> CrushResult<()> {
    loop {
        match input.read() {
            Ok(row) => {
                match evaluate(condition.clone(), &row, input.types(), &env, printer) {
                    Ok(val) => if val { if output.send(row).is_err() { break; } },
                    Err(e) => printer.crush_error(e),
                }
            }
            Err(_) => break,
        }
    }
    Ok(())
}

pub fn parse(_input_type: &Vec<ColumnType>,
             arguments: &mut Vec<Argument>) -> CrushResult<Box<dyn CrushCommand + Send + Sync>> {
    match arguments.remove(0).value {
        Value::Command(c) => Ok(c),
        _ => argument_error("Expected a closure"),
    }
}

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(mut input) => {
            let output = context.output.initialize(input.types().clone())?;
            run(parse(input.types(), context.arguments.as_mut())?,
                input.as_mut(),
                output,
                context.env,
                &context.printer)
        }
        None => error("Expected a stream"),
    }
}
