use crate::lang::command::OutputType::Unknown;
use crate::lang::data::table::{ColumnVec, Row};
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use signature::signature;
use std::collections::HashSet;

#[signature(
    stream.drop,
    can_block = true,
    short = "Drop specified columns from input stream, copy content of remaining columns into output",
    long = "This command is does the opposite of the select command. It copies all columns except the ones specified from input to output.",
    example = "# Drop memory usage columns from output of ps",
    example = "host:procs | drop vms rss",
    output = Unknown,
)]
pub struct Drop {
    #[unnamed()]
    #[description("the columns to remove from the stream.")]
    drop: Vec<String>,
}

fn drop(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Drop::parse(
        context.remove_arguments().clone(),
        &context.global_state.printer(),
    )?;
    let input = context.input.recv()?;
    let mut input = input.stream()?;
    let t = input.types();
    let drop = cfg
        .drop
        .iter()
        .map(|f| t.find(f))
        .collect::<CrushResult<HashSet<usize>>>()?;
    let inc: Vec<bool> = (0..t.len())
        .into_iter()
        .map(|idx| drop.contains(&idx))
        .collect();
    let mut it = inc.iter();
    let output = context.output.initialize(
        &t.to_vec()
            .drain(..)
            .filter(|_| !*(it.next().unwrap()))
            .collect::<Vec<_>>(),
    )?;
    while let Ok(row) = input.read() {
        let mut row = Vec::from(row);
        let mut it = inc.iter();
        output.send(Row::new(
            row.drain(..).filter(|_| !*(it.next().unwrap())).collect(),
        ))?;
    }
    Ok(())
}
