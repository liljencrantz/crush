use crate::lang::command::OutputType::Passthrough;
use crate::lang::data::table::Row;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use signature::signature;

#[signature(
    stream.sample,
    can_block = true,
    output = Passthrough,
    short = "Reservoir-sample `rows` rows from the input.",
    long = "Every row of input has an equal probability of ending up in the output, and the",
    long = "whole stream never has to be materialized to pick them -- only `rows` rows are",
    long = "ever held in memory at once. Output order is not the input order.",
    example = "# Pick 5 pseudo-random lines out of a huge file, without reading it all into memory",
    example = "lines big_log_file.txt | sample 5",
)]
pub struct Sample {
    #[description("the number of rows to sample.")]
    #[default(10)]
    rows: i128,
}

fn sample(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Sample::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut input = context.input_stream()?;
    let output = context.initialize_output(input.types())?;
    let k = cfg.rows.max(0) as usize;

    let mut reservoir: Vec<Row> = Vec::with_capacity(k);
    let mut seen: usize = 0;

    while let Some(row) = input.next_row()? {
        if seen < k {
            reservoir.push(row);
        } else {
            let j = (rand::random::<f64>() * ((seen + 1) as f64)) as usize;
            if j < k {
                reservoir[j] = row;
            }
        }
        seen += 1;
    }

    for row in reservoir {
        output.send(row)?;
    }
    Ok(())
}
