use crate::lang::command::Command;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use signature::signature;
use crate::lang::value::Value;

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

fn r#if(mut context: CommandContext) -> CrushResult<()> {
    let cfg: If = If::parse(context.remove_arguments(), &context.global_state.printer())?;

    if cfg.condition {
        cfg.true_clause.eval(context.with_args(vec![], None))
    } else {
        match cfg.false_clause {
            None => context.output.send(Value::Empty),
            Some(v) => v.eval(context.with_args(vec![], None)),
        }
    }
}
