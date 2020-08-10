use crate::lang::argument::ArgumentHandler;
use crate::lang::command::Command;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::ExecutionContext;
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

fn r#loop(context: ExecutionContext) -> CrushResult<()> {
    let cfg: Loop = Loop::parse(context.arguments.clone(), &context.printer)?;
    context.output.initialize(vec![])?;
    loop {
        let env = context.env.create_child(&context.env, true);
        cfg.body.invoke(ExecutionContext {
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
