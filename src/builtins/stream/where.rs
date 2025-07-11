use crate::lang::ast::location::Location;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Passthrough;
use crate::lang::errors::{CrushResult, error};
use crate::lang::pipe::pipe;
use crate::lang::state::contexts::CommandContext;
use crate::lang::{argument::Argument, data::table::ColumnType};
use crate::lang::{data::table::Row, value::Value};
use signature::signature;

#[signature(
    stream.r#where,
    can_block = true,
    output = Passthrough,
    short = "Filter out rows from input based on condition",
    long = "The columns of the row are exported to the environment using the column names, i.e. if the table the `where` command is applied to has columns `a` and `b`, then the environment will have variables `type` and `name`, the variables `$type` and `$name` will be set to the values of the columns in the current row on each execution of the closure.",
    example = "# List all subdirectories to the current working directory",
    example = "files | where {$type == directory}")]
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
        .map(|(c, t)| Argument::named(t.name(), c, location))
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
    let cfg = Where::parse(context.remove_arguments(), &context.global_state.printer())?;

    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let base_context = context.empty();

            let output = context.output.initialize(input.types())?;
            while let Ok(row) = input.read() {
                match evaluate(
                    cfg.condition.clone(),
                    location,
                    &row,
                    input.types(),
                    &base_context,
                ) {
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
