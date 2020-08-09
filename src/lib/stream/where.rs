use crate::{
    lang::{
        value::Value,
        table::Row,
    },
};
use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::{error, CrushResult};
use crate::lang::stream::{empty_channel, channels, black_hole};
use crate::lang::{table::ColumnType, argument::Argument};
use crate::lang::command::Command;
use signature::signature;
use crate::lang::argument::ArgumentHandler;
use crate::lang::command::OutputType::Passthrough;

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
    base_context: &ExecutionContext) -> CrushResult<bool> {
    let arguments = row.clone().into_vec()
        .drain(..)
        .zip(input_type.iter())
        .map(|(c, t)| Argument::named(t.name.as_ref(), c))
        .collect();

    let (sender, reciever) = channels();

    condition.invoke(base_context.clone().with_args(arguments, None).with_sender(sender))?;

    match reciever.recv()? {
        Value::Bool(b) => Ok(b),
        _ => error("Expected a boolean result")
    }
}

pub fn r#where(context: ExecutionContext) -> CrushResult<()> {
    let cfg: Where = Where::parse(context.arguments, &context.printer)?;

    match context.input.recv()?.stream() {
        Some(mut input) => {
            let base_context = ExecutionContext {
                input: empty_channel(),
                output: black_hole(),
                arguments: vec![],
                env: context.env.clone(),
                this: None,
                printer: context.printer.clone(),
            };
            let output = context.output.initialize(input.types().to_vec())?;
            while let Ok(row) = input.read() {
                match evaluate(cfg.condition.copy(), &row, input.types(), &base_context) {
                    Ok(val) => if val && output.send(row).is_err() { break; },
                    Err(e) => base_context.printer.crush_error(e),
                }
            }
            Ok(())
        }
        None => error("Expected a stream"),
    }
}
