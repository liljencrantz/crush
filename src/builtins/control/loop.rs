use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeType;
use crate::lang::value::ValueType;
use signature::signature;

#[signature(
    control.r#loop,
    condition = true,
    output = Known(ValueType::Empty),
    short = "Repeatedly execute the body until the break command is called.",
    example = "loop {",
    example = "  if $(i_am_tired) {",
    example = "    break",
    example = "  }",
    example = "  echo Working",
    example = "}",
    example = "# loop is real grammar in expression mode too, with the exact same syntax",
    example = "assert (({",
    example = "    $i := 0",
    example = "    $sum := 0",
    example = "    loop {",
    example = "        if ($i >= 5) { break }",
    example = "        $sum = ($sum + $i)",
    example = "        $i = ($i + 1)",
    example = "    }",
    example = "    $sum",
    example = "}()) == 10)",
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
