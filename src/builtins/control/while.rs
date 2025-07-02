use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, data_error};
use crate::lang::pipe::pipe;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeType::Loop;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use signature::signature;

#[signature(
    control.r#while,
    condition = true,
    output = Known(ValueType::Empty),
    short = "Repeatedly execute the body for as long the condition is met.",
    long = "The loop body is optional. If not specified, the condition is executed until it returns false. This effectively means that the condition becomes the body, and the loop break check comes at the end of the loop.",
    example = "while {./some_file:exists} {echo \"hello\"}"
)]
pub struct While {
    #[description("the condition.")]
    condition: Command,
    #[description("the command to invoke as long as the condition is true.")]
    body: Option<Command>,
}

fn r#while(mut context: CommandContext) -> CrushResult<()> {
    let cfg: While = While::parse(context.remove_arguments(), &context.global_state.printer())?;

    loop {
        let (sender, receiver) = pipe();

        let cond_env = context.scope.create_child(&context.scope, Loop);
        cfg.condition.eval(
            context
                .empty()
                .with_scope(cond_env.clone())
                .with_output(sender),
        )?;
        if cond_env.is_stopped() {
            break;
        }

        match receiver.recv()? {
            Value::Bool(true) => match &cfg.body {
                Some(body) => {
                    let body_env = context.scope.create_child(&context.scope, Loop);
                    body.eval(context.empty().with_scope(body_env.clone()))?;
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
    context.output.empty()
}
