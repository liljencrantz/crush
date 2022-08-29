use crate::lang::command::Command;
use crate::lang::errors::{data_error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::value::Value;
use signature::signature;
use crate::lang::pipe::pipe;

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

fn r#while(mut context: CommandContext) -> CrushResult<()> {
    context.output.initialize(vec![])?;
    let cfg: While = While::parse(context.remove_arguments(), &context.global_state.printer())?;

    loop {
        let (sender, receiver) = pipe();

        let cond_env = context.scope.create_child(&context.scope, true);
        cfg.condition.invoke(context.empty().with_output(sender))?;
        if cond_env.is_stopped() {
            break;
        }

        match receiver.recv()? {
            Value::Bool(true) => match &cfg.body {
                Some(body) => {
                    let body_env = context.scope.create_child(&context.scope, true);
                    body.invoke(context.empty())?;
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
