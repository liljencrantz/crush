use crate::lang::command::OutputType::Passthrough;
use crate::lang::data::table::Row;
use crate::lang::errors::{command_error, CrushResult};
use crate::lang::state::contexts::CommandContext;
use signature::signature;
use std::collections::VecDeque;

#[signature(
    stream.tail,
    can_block = true,
    output = Passthrough,
    short = "Return the last row(s) of the input.",
)]
pub struct Tail {
    #[description("the number of rows to return.")]
    #[default(10)]
    rows: i128,
}

fn tail(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Tail::parse(context.remove_arguments(), &context.global_state.printer())?;
    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let output = context.output.initialize(input.types())?;
            let mut q: VecDeque<Row> = VecDeque::new();
            while let Ok(row) = input.read() {
                if q.len() >= cfg.rows as usize {
                    q.pop_front();
                }
                q.push_back(row);
            }
            for row in q.drain(..) {
                output.send(row)?;
            }
            Ok(())
        }
        None => command_error("Expected a stream."),
    }
}
