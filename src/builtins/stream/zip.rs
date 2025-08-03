use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use signature::signature;

#[signature(
    stream.zip,
    can_block = true,
    short = "Combine two streams of data into one containing one row of each input stream in each row of output.",
    long = "If the two streams have different numbers of rows, the longer stream will be truncated to the length of the shorter one.",
    example = "# Prepend an index column to the output of the files command",
    example = "zip $(seq) $(files)"
)]
pub struct Zip {
    #[description("the first stream.")]
    first: Value,
    #[description("the second stream.")]
    second: Value,
}

pub fn zip(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Zip::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut output_type = Vec::new();
    let mut first = cfg.first.stream(context.command_handle())?;
    let mut second = cfg.second.stream(context.command_handle())?;
    output_type.append(&mut first.types().to_vec());
    output_type.append(&mut second.types().to_vec());
    let output = context.output.initialize(&output_type)?;
    while let (Ok(mut row1), Ok(row2)) = (first.read(), second.read()) {
        row1.append(&mut Vec::from(row2));
        output.send(row1)?;
    }
    Ok(())
}
