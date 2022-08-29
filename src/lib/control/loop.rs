use crate::lang::command::Command;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::CommandContext;
use signature::signature;

#[signature(
    r#loop,
    condition = true,
    short = "Repeatedly execute the body until the break command is called.",
    example = "loop {\n        if (i_am_tired) {\n            break\n        }\n        echo \"Working\"\n    }"
)]
pub struct Loop {
    #[description("the command to repeatedly invoke.")]
    body: Command,
}

fn r#loop(context: CommandContext) -> CrushResult<()> {
    let cfg: Loop = Loop::parse(context.arguments.clone(), &context.global_state.printer())?;
    context.output.initialize(vec![])?;
    loop {
        let env = context.scope.create_child(&context.scope, true);
        cfg.body.invoke(context.empty())?;
        if env.is_stopped() {
            break;
        }
    }
    Ok(())
}
