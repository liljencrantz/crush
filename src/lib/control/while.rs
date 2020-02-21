use crate::{
    data::Argument,
    data::Value,
};
use crate::printer::Printer;
use crate::namespace::Namespace;
use crate::data::{Stream, RowsReader, ListReader, Struct, DictReader};
use crate::errors::{argument_error, CrushResult, data_error};
use crate::closure::Closure;
use crate::lib::ExecutionContext;
use crate::stream::{empty_channel, Readable, channels};
use crate::stream_printer::spawn_print_thread;

pub struct Config {
    condition: Closure,
    body: Closure,
    env: Namespace,
    printer: Printer,
}

pub fn run(mut config: Config) -> CrushResult<()> {
    let env = config.env.create_child(&config.env, true);
    loop {
        let (sender, receiver) = channels();

        config.condition.spawn_and_execute(ExecutionContext {
            input: empty_channel(),
            output: sender,
            arguments: Vec::new(),
            env: config.env.clone(),
            printer: config.printer.clone(),
        });

        match receiver.recv()? {
            Value::Bool(true) => {
                config.body.spawn_and_execute(ExecutionContext {
                    input: empty_channel(),
                    output: spawn_print_thread(&config.printer),
                    arguments: Vec::new(),
                    env: env.clone(),
                    printer: config.printer.clone(),
                });
                if env.is_stopped() {
                    break;
                }
            }
            Value::Bool(false) => break,
            _ => return data_error("While loop condition must be of boolean type"),
        }
    }
    Ok(())
}

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.initialize(vec![])?;

    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }

    match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
        (Value::Closure(condition), Value::Closure(body)) =>
            run(Config {
                body,
                condition,
                env: context.env,
                printer: context.printer,
            }),
        _ => argument_error("While command expects two closures as arguments"),
    }
}
