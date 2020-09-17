use crate::lang::command::Command;
use crate::lang::errors::{data_error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::stream::{black_hole, channels, empty_channel};
use crate::lang::value::Value;
use signature::signature;

#[signature(
    r#while,
    condition = true,
    short = "Repeatedly execute the body for as long the condition is met.",
    long = "The loop body is optional. If not specified, the condition is executed until it returns false.\n    This effectively means that the condition becomes the body, and the loop break check comes at\n    the end of the loop.",
    example = "while {./some_file:exists} {echo \"hello\"}"
)]
pub struct While {
    #[description("the condition.")]
    condition: Command,
    #[description("the command to invoke as long as the condition is true.")]
    body: Option<Command>,
}

fn r#while(context: CommandContext) -> CrushResult<()> {
    context.output.initialize(vec![])?;
    let cfg: While = While::parse(context.arguments, &context.printer)?;

    loop {
        let (sender, receiver) = channels();

        let cond_env = context.scope.create_child(&context.scope, true);
        cfg.condition.invoke(CommandContext {
            input: empty_channel(),
            output: sender,
            arguments: Vec::new(),
            scope: cond_env.clone(),
            this: None,
            printer: context.printer.clone(),
            threads: context.threads.clone(),
            global_state: context.global_state.clone(),
        })?;
        if cond_env.is_stopped() {
            break;
        }

        match receiver.recv()? {
            Value::Bool(true) => match &cfg.body {
                Some(body) => {
                    let body_env = context.scope.create_child(&context.scope, true);
                    body.invoke(CommandContext {
                        input: empty_channel(),
                        output: black_hole(),
                        arguments: Vec::new(),
                        scope: body_env.clone(),
                        this: None,
                        printer: context.printer.clone(),
                        threads: context.threads.clone(),
                        global_state: context.global_state.clone(),
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
