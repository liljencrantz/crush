use crate::lang::command::Command;
use crate::lang::errors::{argument_error_legacy, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeType;
use crate::lang::value::Value;
use signature::signature;

#[signature(
    control.r#if,
    condition = true,
    short = "Conditionally execute a command once.",
    example = "if $(gt $a 10) {",
    example = "  echo big",
    example = "} else {",
    example = "  echo small",
    example = "}",
)]
pub struct If {
    #[description("the condition to filter on.")]
    condition: bool,
    #[description("the command to invoke if the condition is true.")]
    true_clause: Command,
    #[default("else")]
    r#else: String,
    #[description("the (optional) command to invoke if the condition is false.")]
    false_clause: Option<Command>,
}

fn r#if(mut context: CommandContext) -> CrushResult<()> {
    let cfg: If = If::parse(context.remove_arguments(), &context.global_state.printer())?;

    if cfg.condition {
        let env = context
            .scope
            .create_child(&context.scope, ScopeType::Conditional);
        cfg.true_clause
            .eval(context.empty().with_scope(env).with_output(context.output))
    } else {
        match (cfg.false_clause, cfg.r#else.as_str()) {
            (None, "else") => context.output.send(Value::Empty),
            (Some(v), "else") => {
                let env = context
                    .scope
                    .create_child(&context.scope, ScopeType::Conditional);
                v.eval(context.empty().with_scope(env).with_output(context.output))
            }
            _ => argument_error_legacy("Invalid else-clause. Did you misspell else?")
        }
    }
}
