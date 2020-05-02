use crate::lang::errors::CrushResult;
use crate::lang::execution_context::ArgumentVector;
use crate::lang::execution_context::ExecutionContext;
use crate::lang::stream::{black_hole, empty_channel};

pub fn r#loop(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.initialize(vec![])?;
    context.arguments.check_len(1)?;
    let body = context.arguments.command(0)?;
    loop {
        let env = context.env.create_child(&context.env, true);
        body.invoke(ExecutionContext {
            input: empty_channel(),
            output: black_hole(),
            arguments: Vec::new(),
            env: env.clone(),
            this: None,
            printer: context.printer.clone(),
        })?;
        if env.is_stopped() {
            break;
        }
    }
    Ok(())
}
