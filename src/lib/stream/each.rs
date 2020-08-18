use crate::lang::argument::ArgumentHandler;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Passthrough;
use crate::lang::errors::{error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::stream::{black_hole, channels, empty_channel};
use crate::lang::{argument::Argument, table::ColumnType};
use crate::lang::{table::Row, value::Value};
use signature::signature;
use crate::lang::value::ValueType::Empty;
use crate::lang::command::OutputType::Known;

#[signature(
r#each,
can_block = true,
output = Known(Empty),
short = "Runs a command one for each row of input",
long = "The columns of the row are exported to the environment using the column names.",
example = "ps | where {status != \"Sleeping\"} | each {echo (\"{} is sleepy\":format name)}")]
pub struct Each {
    #[description("the command to run.")]
    body: Command,
}

fn run(
    condition: Command,
    row: &Row,
    input_type: &[ColumnType],
    base_context: &CommandContext,
) -> CrushResult<()> {
    let arguments = row
        .clone()
        .into_vec()
        .drain(..)
        .zip(input_type.iter())
        .map(|(c, t)| Argument::named(t.name.as_ref(), c))
        .collect();

    condition.invoke(
        base_context
            .clone()
            .with_args(arguments, None)
    )
}

pub fn each(context: CommandContext) -> CrushResult<()> {
    let cfg: Each = Each::parse(context.arguments, &context.printer)?;
    context.output.send(Value::Empty())?;

    match context.input.recv()?.stream() {
        Some(mut input) => {
            let base_context = CommandContext {
                input: empty_channel(),
                output: black_hole(),
                arguments: vec![],
                scope: context.scope.clone(),
                this: None,
                printer: context.printer.clone(),
            };

            while let Ok(row) = input.read() {
                match run(cfg.body.copy(), &row, input.types(), &base_context) {
                    Ok(_) => (),
                    Err(e) => base_context.printer.crush_error(e),
                }
            }
            Ok(())
        }
        None => error("Expected a stream"),
    }
}
