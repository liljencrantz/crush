use crate::lang::errors::{CrushResult, argument_error_legacy};
use crate::lang::state::contexts::CommandContext;
use signature::signature;
use crate::lang::command::OutputType::Passthrough;

#[signature(
head,
can_block = true,
output = Passthrough,
short = "Return the first row(s) of the input.",
)]
pub struct Head {
    #[description("the number of rows to return.")]
    #[default(10)]
    rows: i128,
}

fn head(context: CommandContext) -> CrushResult<()> {
    let cfg: Head = Head::parse(context.arguments, &context.global_state.printer())?;
    match context.input.recv()?.stream()? {
        Some(mut input) => {
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
        None => argument_error_legacy("Expected a stream"),
    }
}
