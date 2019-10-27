use crate::{
    data::Argument,
    data::Row,
    data::CellType,
    stream::{OutputStream, InputStream},
    data::Cell,
    errors::JobError,
    env::get_cwd,
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::{ColumnType, JobOutput};
use crate::errors::{argument_error, JobResult};
use crate::closure::ClosureDefinition;
use crate::commands::CompileContext;
use crate::stream::{spawn_print_thread, empty_stream};
use std::sync::atomic::Ordering::AcqRel;

pub struct Config {
    iter: JobOutput,
    body: ClosureDefinition,
    env: Env,
    printer: Printer,
}

pub fn parse(mut context: CompileContext) -> Result<Config, JobError> {
    context.input.initialize()?;
    context.output.initialize(vec![])?;

    if context.arguments.len() != 2 {
        return Err(argument_error("Expected exactly two arguments"));
    }

    let mut it = context.arguments.drain(..);
    match (it.next().unwrap().cell, it.next().unwrap().cell) {
        (Cell::JobOutput(o), Cell::ClosureDefinition(c)) => {
            Ok(Config {
                iter: o,
                body: c,
                env: context.env,
                printer: context.printer,
            })
        }
        _ => Err(argument_error("First argument to for must be a job, the second must be a closure")),
    }
}

pub fn run(config: Config, ) -> JobResult<()> {
    loop {
        match config.iter.stream.recv() {
            Ok(mut line) => {
                let arguments = line.cells
                    .drain(..)
                    .zip(config.iter.stream.get_type().iter())
                    .map(|(c, t)| {
                        match &t.name {
                            None => Argument::unnamed(c),
                            Some(name) => Argument::named(name.as_ref(), c),
                        }
                    })
                    .collect();
                config.body.spawn_and_execute(CompileContext{
                    input: empty_stream(),
                    output: spawn_print_thread(&config.printer),
                    arguments,
                    env: config.env.clone(),
                    printer: config.printer.clone(),
                });
            },
            Err(_) => {break;},
        }
    }
    Ok(())
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let config = parse(context)?;
    run(config)
}
