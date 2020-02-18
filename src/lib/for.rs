use crate::{
    data::Argument,
    data::Value,
};
use crate::printer::Printer;
use crate::namepspace::Namespace;
use crate::data::{Stream};
use crate::errors::{argument_error, CrushResult};
use crate::closure::Closure;
use crate::lib::ExecutionContext;
use crate::stream::empty_channel;
use crate::stream_printer::spawn_print_thread;

pub struct Config {
    iter: Stream,
    body: Closure,
    env: Namespace,
    printer: Printer,
}

pub fn parse(mut context: ExecutionContext) -> CrushResult<Config> {
    context.output.initialize(vec![])?;

    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }

    let mut it = context.arguments.drain(..);
    match (it.next().unwrap().value, it.next().unwrap().value) {
        (Value::Stream(o), Value::Closure(c)) => {
            Ok(Config {
                iter: o,
                body: c,
                env: context.env,
                printer: context.printer,
            })
        }
        _ => argument_error("First argument to for must be a job, the second must be a closure"),
    }
}

pub fn run(config: Config) -> CrushResult<()> {
    loop {
        match config.iter.stream.recv() {
            Ok(mut line) => {
                let arguments = line.into_vec()
                    .drain(..)
                    .zip(config.iter.stream.types().iter())
                    .map(|(c, t)| {
                        match &t.name {
                            None => Argument::unnamed(c),
                            Some(name) => Argument::named(name.as_ref(), c),
                        }
                    })
                    .collect();
                config.body.spawn_and_execute(ExecutionContext {
                    input: empty_channel(),
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

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    let config = parse(context)?;
    run(config)
}
