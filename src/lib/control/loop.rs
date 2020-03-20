use crate::{
    lang::argument::Argument,
    lang::value::Value,
};
use crate::lang::scope::Scope;
use crate::lang::{table::TableReader, list::ListReader, r#struct::Struct, dict::DictReader, command::CrushCommand};
use crate::lang::errors::{argument_error, CrushResult, data_error};
use crate::lang::command::Closure;
use crate::lang::command::ExecutionContext;
use crate::lang::stream::{empty_channel, Readable, channels};
use crate::lang::stream_printer::spawn_print_thread;
use crate::lib::parse_util::single_argument_closure;

pub fn run(body: Closure, parent: Scope) -> CrushResult<()> {
    let env = parent.create_child(&parent, true);
    loop {
        body.invoke(ExecutionContext {
            input: empty_channel(),
            output: spawn_print_thread(),
            arguments: Vec::new(),
            env: env.clone(),
            this: None,
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
    run(body, context.env)
}
