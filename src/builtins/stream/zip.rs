use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::pipe::Stream;
use signature::signature;

#[signature(
    stream.zip,
    can_block = true,
    short = "Combine two streams of data into one containing one row of each input stream in each row of output.",
    long = "If the two streams have different numbers of rows, the longer stream will be truncated",
    long = "to the length of the shorter one.",
    example = "# Prepend an index column to the output of the files command",
    example = "zip $(seq) $(files)"
)]
pub struct Zip {
    #[description("the first stream.")]
    first: Stream,
    #[description("the second stream.")]
    second: Stream,
}

pub fn zip(context: CommandContext) -> CrushResult<()> {
    let mut cfg = Zip::parse(context.arguments, &context.global_state.printer())?;
    let mut output_type = Vec::new();
    output_type.append(&mut cfg.first.types().to_vec());
    output_type.append(&mut cfg.second.types().to_vec());
    let output = context.output.initialize(&output_type)?;
    while let (Ok(mut row1), Ok(row2)) = (cfg.first.read(), cfg.second.read()) {
        row1.append(&mut Vec::from(row2));
        output.send(row1)?;
    }
    Ok(())
}
