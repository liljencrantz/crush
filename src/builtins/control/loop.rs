use crate::lang::command::Command;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ValueType;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::pipe::pipe;
use crate::lang::command::OutputType::Known;
use signature::signature;
use crate::lang::state::scope::ScopeType;
use crate::lang::value::Value::Empty;

#[signature(
    control.r#loop,
    condition = true,
    output = Known(ValueType::Empty),
    short = "Repeatedly execute the body until the break command is called.",
    example = "loop {\n        if (i_am_tired) {\n            break\n        }\n        echo \"Working\"\n    }"
)]
pub struct Loop {
    #[description("the command to repeatedly invoke.")]
    body: Command,
}

fn r#loop(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Loop = Loop::parse(context.remove_arguments(), &context.global_state.printer())?;
    loop {
        let env = context.scope.create_child(&context.scope, ScopeType::Loop);
        cfg.body.eval(context.empty().with_scope(env.clone()))?;
        if env.is_stopped() {
            context.output.empty()?;
            break;
        }
    }
    Ok(())
}
