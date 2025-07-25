use crate::lang::command::OutputType::Passthrough;
use crate::lang::data::table::Row;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use signature::signature;

#[signature(
    stream.reverse,
    can_block = true,
    short = "Reverses the order of the rows in the input",
    output = Passthrough)]
pub struct Reverse {}

fn reverse(mut context: CommandContext) -> CrushResult<()> {
    Reverse::parse(
        context.remove_arguments().clone(),
        &context.global_state.printer(),
    )?;
    let mut input = context.input.recv()?.stream()?;
    let output = context.output.initialize(input.types())?;
    let mut q: Vec<Row> = Vec::new();
    while let Ok(row) = input.read() {
        q.push(row);
    }
    while !q.is_empty() {
        output.send(q.pop().unwrap())?;
    }
    Ok(())
}
