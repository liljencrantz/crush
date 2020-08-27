use crate::lang::argument::ArgumentHandler;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Passthrough;
use crate::lang::errors::{error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::stream::{black_hole, channels, empty_channel};
use crate::lang::{argument::Argument, table::ColumnType};
use crate::lang::{table::Row, value::Value};
use signature::signature;

#[signature(
r#where,
can_block = true,
output = Passthrough,
short = "Filter out rows from io based on condition",
long = "The columns of the row are exported to the environment using the column names.",
example = "ps | where {status != \"Sleeping\"}")]
pub struct Where {
    #[description("the condition to filter on.")]
    condition: Command,
}

fn evaluate(
    condition: Command,
    row: &Row,
    input_type: &[ColumnType],
    base_context: &CommandContext,
) -> CrushResult<bool> {
    let arguments = row
        .clone()
        .into_vec()
        .drain(..)
        .zip(input_type.iter())
        .map(|(c, t)| Argument::named(t.name.as_ref(), c))
        .collect();

    let (sender, reciever) = channels();

    condition.invoke(
        base_context
            .clone()
            .with_args(arguments, None)
            .with_output(sender),
    )?;

    match reciever.recv()? {
        Value::Bool(b) => Ok(b),
        _ => error("Expected a boolean result"),
    }
}

pub fn r#where(context: CommandContext) -> CrushResult<()> {
    let cfg: Where = Where::parse(context.arguments, &context.printer)?;

    match context.input.recv()?.stream() {
        Some(mut input) => {
            let base_context = CommandContext {
                input: empty_channel(),
                output: black_hole(),
                arguments: vec![],
                scope: context.scope.clone(),
                this: None,
                printer: context.printer.clone(),
                threads: context.threads.clone(),
            };

            let output = context.output.initialize(input.types().to_vec())?;
            while let Ok(row) = input.read() {
                match evaluate(cfg.condition.copy(), &row, input.types(), &base_context) {
                    Ok(val) => {
                        if val && output.send(row).is_err() {
                            break;
                        }
                    }
                    Err(e) => base_context.printer.crush_error(e),
                }
            }
            Ok(())
        }
        None => error("Expected a stream"),
    }
}
