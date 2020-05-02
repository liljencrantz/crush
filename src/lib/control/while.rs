use crate::lang::errors::{data_error, CrushResult};
use crate::lang::execution_context::{ArgumentVector, ExecutionContext};
use crate::lang::stream::{black_hole, channels, empty_channel};
use crate::lang::value::Value;

pub fn r#while(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.initialize(vec![])?;
    context.arguments.check_len_range(1, 2)?;

    let condition = context.arguments.command(0)?;
    let maybe_body = context.arguments.optional_command(1)?;
    loop {
        let (sender, receiver) = channels();

        let cond_env = context.env.create_child(&context.env, true);
        condition.invoke(ExecutionContext {
            input: empty_channel(),
            output: sender,
            arguments: Vec::new(),
            env: cond_env.clone(),
            this: None,
            printer: context.printer.clone(),
        })?;
        if cond_env.is_stopped() {
            break;
        }

        match receiver.recv()? {
            Value::Bool(true) => match &maybe_body {
                Some(body) => {
                    let body_env = context.env.create_child(&context.env, true);
                    body.invoke(ExecutionContext {
                        input: empty_channel(),
                        output: black_hole(),
                        arguments: Vec::new(),
                        env: body_env.clone(),
                        this: None,
                        printer: context.printer.clone(),
                    })?;
                    if body_env.is_stopped() {
                        break;
                    }
                }
                None => {}
            },
            Value::Bool(false) => break,
            _ => return data_error("While loop condition must output value of boolean type"),
        }
    }
    Ok(())
}
