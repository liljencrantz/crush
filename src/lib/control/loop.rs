use crate::{
    lang::argument::Argument,
    lang::value::Value,
};
use crate::lang::scope::Scope;
use crate::lang::{table::TableReader, list::ListReader, r#struct::Struct, dict::DictReader, command::CrushCommand};
use crate::lang::errors::{argument_error, CrushResult, data_error};
use crate::lang::command::{This, ArgumentVector};
use crate::lang::command::ExecutionContext;
use crate::lang::stream::{empty_channel, Readable, channels};
use crate::lang::stream_printer::spawn_print_thread;

pub fn run(body: Box<dyn CrushCommand>, parent: Scope) -> CrushResult<()> {
    let env = parent.create_child(&parent, true);
    loop {
        body.invoke(ExecutionContext {
            input: empty_channel(),
            output: spawn_print_thread(),
            arguments: Vec::new(),
            env: env.clone(),
            this: None,
        })?;
        if env.is_stopped() {
            break;
        }
    }
    Ok(())
}

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.initialize(vec![])?;
    let body = context.arguments.command(0)?;
    run(body, context.env)
}
