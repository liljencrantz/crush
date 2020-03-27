use crate::lang::scope::Scope;
use crate::lang::command::CrushCommand;
use crate::lang::errors::CrushResult;
use crate::lang::command::{ArgumentVector};
use crate::lang::command::ExecutionContext;
use crate::lang::stream::empty_channel;
use crate::lang::pretty_printer::spawn_print_thread;

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
