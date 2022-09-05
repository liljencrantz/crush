use crate::lang::command::Command;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::CommandContext;
use crate::lang::value::ValueType;
use crate::lang::data::table::ColumnType;
use lazy_static::lazy_static;
use crate::data::table::Row;
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

fn r#loop(context: CommandContext) -> CrushResult<()> {
    let cfg: Loop = Loop::parse(context.arguments.clone(), &context.global_state.printer())?;
    let output = context.output.initialize(OUTPUT_TYPE.clone())?;
    let (sender, receiver) = pipe();
    loop {
        let env = context.scope.create_child(&context.scope, true);
        cfg.body.eval(context.empty().with_scope(env.clone()).with_output(sender.clone()))?;
        output.send(Row::new(vec![receiver.recv()?]))?;
        if env.is_stopped() {
            break;
        }
    }
    Ok(())
}
