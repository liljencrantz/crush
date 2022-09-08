use crate::lang::command::Command;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::CommandContext;
use crate::lang::value::ValueType;
use crate::lang::data::table::ColumnType;
use lazy_static::lazy_static;
use crate::lang::pipe::pipe;
use crate::lang::command::OutputType::Known;
use signature::signature;

lazy_static! {
    static ref OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("value", ValueType::Any),
    ];
}

#[signature(
    r#loop,
    condition = true,
    output = Known(ValueType::TableInputStream(OUTPUT_TYPE.clone())),
    short = "Repeatedly execute the body until the break command is called.",
    example = "loop {\n        if (i_am_tired) {\n            break\n        }\n        echo \"Working\"\n    }"
)]
pub struct Loop {
    #[description("the command to repeatedly invoke.")]
    body: Command,
}

fn r#loop(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Loop = Loop::parse(context.remove_arguments(), &context.global_state.printer())?;
    let (sender, receiver) = pipe();
    loop {
        let env = context.scope.create_child(&context.scope, true);
        cfg.body.eval(context.empty().with_scope(env.clone()).with_output(sender.clone()))?;
        if env.is_stopped() {
            context.output.send(receiver.recv()?)?;
            break;
        }
        receiver.recv()?;
    }
    Ok(())
}
