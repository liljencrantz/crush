use crate::{
    lang::argument::Argument,
    lang::value::Value,
};
use crate::printer::Printer;
use crate::scope::Scope;
use crate::lang::{table::TableStream, table::TableReader, list::ListReader, r#struct::Struct, dict::DictReader, command::CrushCommand};
use crate::errors::{argument_error, CrushResult, data_error};
use crate::lang::closure::Closure;
use crate::lang::command::ExecutionContext;
use crate::stream::{empty_channel, Readable, channels};
use crate::stream_printer::spawn_print_thread;
use crate::lib::parse_util::single_argument_closure;

pub fn run(body: Closure, parent: Scope, printer: Printer) -> CrushResult<()> {
    let env = parent.create_child(&parent, true);
    loop {
        body.invoke(ExecutionContext {
            input: empty_channel(),
            output: spawn_print_thread(&printer),
            arguments: Vec::new(),
            env: env.clone(),
            printer: printer.clone(),
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
    run(body, context.env, context.printer)
}
