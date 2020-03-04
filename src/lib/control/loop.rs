use crate::{
    lang::Argument,
    lang::Value,
};
use crate::printer::Printer;
use crate::scope::Scope;
use crate::lang::{Stream, RowsReader, ListReader, Struct, DictReader, CrushCommand};
use crate::errors::{argument_error, CrushResult, data_error};
use crate::lang::Closure;
use crate::lang::ExecutionContext;
use crate::stream::{empty_channel, Readable, channels};
use crate::stream_printer::spawn_print_thread;
use crate::lib::parse_util::single_argument_closure;

pub struct Config {
    body: Closure,
    env: Scope,
    printer: Printer,
}

pub fn run(mut config: Config) -> CrushResult<()> {
    let env = config.env.create_child(&config.env, true);
    loop {
        config.body.invoke(ExecutionContext {
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
    Ok(())
}

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.initialize(vec![])?;
    let body = single_argument_closure(context.arguments)?;
        run(Config {
            body,
            env: context.env,
            printer: context.printer,
        })
}
