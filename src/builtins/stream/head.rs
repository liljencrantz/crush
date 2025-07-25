use crate::lang::command::OutputType::Passthrough;
use crate::lang::errors::{CrushResult};
use crate::lang::state::contexts::CommandContext;
use signature::signature;

#[signature(
    stream.head,
    can_block = true,
    output = Passthrough,
    short = "Return the first row(s) of the input.",
)]
pub struct Head {
    #[description("the number of rows to return.")]
    #[default(10)]
    rows: i128,
}

fn head(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Head::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut input = context.input.recv()?.stream()?;
    let output = context.output.initialize(input.types())?;
    let mut count = 0;
    while let Ok(row) = input.read() {
        if count >= cfg.rows {
            break;
        }
        output.send(row)?;
        count += 1;
    }
    Ok(())
}
