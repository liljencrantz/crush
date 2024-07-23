use crate::lang::command::Command;
use crate::lang::command::OutputType::Passthrough;
use crate::lang::errors::{error, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::{argument::Argument, data::table::ColumnType};
use crate::lang::{data::table::Row, value::Value};
use signature::signature;
use crate::lang::ast::location::Location;
use crate::lang::pipe::pipe;

#[signature(
    stream.r#where,
    can_block = true,
    output = Passthrough,
    short = "Filter out rows from io based on condition",
    long = "The columns of the row are exported to the environment using the column names.",
    example = "host:procs | where {status != \"Sleeping\"}")]
pub struct Where {
    #[description("the condition to filter on.")]
    condition: Command,
}

fn evaluate(
    condition: Command,
    location: Location,
    row: &Row,
    input_type: &[ColumnType],
    base_context: &CommandContext,
) -> CrushResult<bool> {
    let arguments = Vec::from(row.clone())
        .drain(..)
        .zip(input_type.iter())
        .map(|(c, t)| Argument::named(t.name.as_ref(), c, location))
        .collect();

    let (sender, reciever) = pipe();

    condition.eval(
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

pub fn r#where(mut context: CommandContext) -> CrushResult<()> {
    let location = context.arguments[0].location;
    let cfg: Where = Where::parse(context.remove_arguments(), &context.global_state.printer())?;

    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let base_context = context.empty();

            let output = context.output.initialize(input.types())?;
            while let Ok(row) = input.read() {
                match evaluate(cfg.condition.clone(), location, &row, input.types(), &base_context) {
                    Ok(val) => {
                        if val && output.send(row).is_err() {
                            break;
                        }
                    }
                    Err(e) => base_context.global_state.printer().crush_error(e),
                }
            }
            Ok(())
        }
        None => error("Expected a stream"),
    }
}
