use crate::lang::command::Command;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::CommandContext;
use signature::signature;

#[signature(
    r#if,
    condition = true,
    short = "Conditionally execute a command once.",
    example = "if a > 10 {echo \"big\"} {echo \"small\"}"
)]
pub struct If {
    #[description("the condition to filter on.")]
    condition: bool,
    #[description("the command to invoke if the condition is true.")]
    true_clause: Command,
    #[description("the (optional) command to invoke if the condition is false.")]
    false_clause: Option<Command>,
}

fn r#if(context: CommandContext) -> CrushResult<()> {
    let cfg: If = If::parse(context.arguments.clone(), &context.printer)?;

    if cfg.condition {
        cfg.true_clause.invoke(context.with_args(vec![], None))
    } else {
        cfg.false_clause
            .map(|v| v.invoke(context.with_args(vec![], None)))
            .unwrap_or(Ok(()))
    }
}
