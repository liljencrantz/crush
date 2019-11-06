use crate::{
    data::Argument,
    data::Cell,
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::{JobOutput};
use crate::errors::{argument_error, JobResult};
use crate::closure::Closure;
use crate::commands::CompileContext;
use crate::stream::empty_stream;
use crate::stream_printer::spawn_print_thread;

pub struct Config {
    iter: JobOutput,
    body: Closure,
    env: Env,
    printer: Printer,
}

pub fn parse(mut context: CompileContext) -> JobResult<Config> {
    context.input.initialize()?;
    context.output.initialize(vec![])?;

    if context.arguments.len() != 2 {
        return Err(argument_error("Expected exactly two arguments"));
    }

    let mut it = context.arguments.drain(..);
    match (it.next().unwrap().cell, it.next().unwrap().cell) {
        (Cell::Output(o), Cell::Closure(c)) => {
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

pub fn run(config: Config) -> JobResult<()> {
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
