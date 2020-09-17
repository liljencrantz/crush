use crate::lang::command::Command;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::CommandContext;
use crate::lang::stream::{black_hole, empty_channel};
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
    let cfg: Loop = Loop::parse(context.arguments.clone(), &context.printer)?;
    context.output.initialize(vec![])?;
    loop {
        let env = context.scope.create_child(&context.scope, true);
        cfg.body.invoke(CommandContext {
            input: empty_channel(),
            output: black_hole(),
            arguments: Vec::new(),
            scope: env.clone(),
            this: None,
            printer: context.printer.clone(),
            threads: context.threads.clone(),
            global_state: context.global_state.clone(),
        })?;
        if env.is_stopped() {
            break;
        }
    }
    Ok(())
}
