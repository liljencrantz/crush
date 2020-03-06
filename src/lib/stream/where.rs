use std::cmp::Ordering;

use crate::{
    lang::{
        value::Value,
        table::Row,
    },
    lang::stream::{OutputStream}
};
use crate::lang::command::ExecutionContext;
use crate::lang::errors::{error, CrushResult, argument_error};
use crate::lang::printer::Printer;
use crate::lang::stream::{Readable, empty_channel, channels, ValueSender};
use crate::lang::{table::TableReader, table::ColumnType, argument::Argument};
use crate::lang::command::Closure;
use crate::lang::stream_printer::spawn_print_thread;
use crate::lang::scope::Scope;
use crate::lang::command::CrushCommand;

fn evaluate(condition: &Closure, row: &Row, input_type: &Vec<ColumnType>, env: &Scope, printer: &Printer) -> CrushResult<bool> {
    let arguments = row.clone().into_vec()
        .drain(..)
        .zip(input_type.iter())
        .map(|(c, t)| {
            match &t.name {
                None => Argument::unnamed(c.clone()),
                Some(name) => Argument::named(name.as_ref(), c),
            }
        })
        .collect();

    let (sender, reciever) = channels();

    condition.invoke(ExecutionContext {
        input: empty_channel(),
        output: sender,
        arguments,
        env: env.clone(),
        printer: printer.clone(),
    });

    match reciever.recv()? {
        Value::Bool(b) => Ok(b),
        _ => error("Expected a boolean result")
    }
}

pub fn run(mut condition: &Closure, input: &mut dyn Readable, output: OutputStream, env: Scope, printer: Printer) -> CrushResult<()> {
    loop {
        match input.read() {
            Ok(row) => {
                match evaluate(condition, &row, input.types(), &env, &printer) {
                    Ok(val) => if val { if output.send(row).is_err() { break }},
                    Err(e) => printer.job_error(e),
                }
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn parse(_input_type: &Vec<ColumnType>,
             arguments: &mut Vec<Argument>) -> CrushResult<Closure> {
    match arguments.remove(0).value {
        Value::Closure(c) => Ok(c),
        _ => argument_error("Expected a closure"),
    }
}

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(mut input) => {
            let output = context.output.initialize(input.types().clone())?;
            run(&parse(input.types(), context.arguments.as_mut())?,
                input.as_mut(),
                output,
                context.env,
                context.printer)
        }
        None => error("Expected a stream"),
    }
}
