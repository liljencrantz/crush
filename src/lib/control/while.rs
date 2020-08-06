use crate::lang::value::Value;
use crate::lang::errors::{CrushResult, data_error};
use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use crate::lang::stream::{empty_channel, channels, black_hole};
use signature::signature;
use crate::lang::argument::ArgumentHandler;
use crate::lang::command::Command;

#[signature(
r#while,
condition = true,
short = "Repeatedly execute the body for as long the condition is met.",
long = "The loop body is optional. If not specified, the condition is executed until it returns false.\n    This effectively means that the condition becomes the body, and the loop break check comes at\n    the end of the loop.",
example = "while {./some_file:exists} {echo \"hello\"}")]
pub struct While {
    #[description("the condition.")]
    condition: Command,
    #[description("the command to invoke as long as the condition is true.")]
    body: Option<Command>,
}

fn r#while(context: ExecutionContext) -> CrushResult<()> {
    context.output.initialize(vec![])?;
    let cfg: While = While::parse(context.arguments, &context.printer)?;

    loop {
        let (sender, receiver) = channels();

        let cond_env = context.env.create_child(&context.env, true);
        cfg.condition.invoke(ExecutionContext {
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
            Value::Bool(true) => {
                match &cfg.body {
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
                }
            }
            Value::Bool(false) => break,
            _ => return data_error("While loop condition must output value of boolean type"),
        }
    }
    Ok(())
}
