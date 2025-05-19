use crate::lang::errors::argument_error_legacy;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use signature::signature;
use crate::lang::command::OutputType::Passthrough;
#[signature(
    stream.skip,
    can_block = true,
    output = Passthrough,
    short = "Skip the specified number of rows in the stream and return the remainder.",
)]
pub struct Skip {
    #[description("the number of rows to skip.")]
    #[default(1)]
    rows: i128,
}

fn skip(context: CommandContext) -> CrushResult<()> {
    let cfg: Skip = Skip::parse(context.arguments, &context.global_state.printer())?;
    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let output = context.output.initialize(input.types())?;
            let mut res: i128 = 0;
            while res < cfg.rows {
                if let Err(_) = input.read() {
                    return Ok(())
                }
                res += 1;
            }
            while let Ok(row) = input.read() {
                output.send(row)?;
            }
            Ok(())
        }
        None => argument_error_legacy("Expected a stream"),
    }
}
