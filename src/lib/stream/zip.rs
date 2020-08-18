use crate::lang::argument::ArgumentHandler;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::CommandContext;
use crate::lang::stream::Stream;
use signature::signature;

#[signature(zip, can_block = true, short = "Combine two streams of data into one")]
pub struct Zip {
    #[description("the first stream.")]
    first: Stream,
    #[description("the second stream.")]
    second: Stream,
}

pub fn zip(context: CommandContext) -> CrushResult<()> {
    let mut cfg: Zip = Zip::parse(context.arguments, &context.printer)?;
    let mut output_type = Vec::new();
    output_type.append(&mut cfg.first.types().to_vec());
    output_type.append(&mut cfg.second.types().to_vec());
    let output = context.output.initialize(output_type)?;
    while let (Ok(mut row1), Ok(row2)) = (cfg.first.read(), cfg.second.read()) {
        row1.append(&mut row2.into_vec());
        output.send(row1)?;
    }
    Ok(())
}
