use crate::lang::ast::source::Source;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, error};
use crate::lang::pipe::pipe;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ValueType;
use crate::lang::{argument::Argument, data::table::ColumnType};
use crate::lang::{data::table::Row, value::Value};
use signature::signature;

/// Shared by `any`/`all`: evaluate `condition` against one row, the same way `where`
/// does -- the row's own columns are exported to the closure by name.
fn evaluate(
    condition: &Command,
    source: &Source,
    row: &Row,
    input_type: &[ColumnType],
    base_context: &CommandContext,
) -> CrushResult<bool> {
    let arguments = Vec::from(row.clone())
        .drain(..)
        .zip(input_type.iter())
        .map(|(c, t)| Argument::named(t.name(), c, source))
        .collect();

    let (sender, receiver) = pipe();

    condition.eval(
        base_context
            .clone()
            .with_args(arguments, None)
            .with_output(sender),
    )?;

    match receiver.recv()? {
        Value::Bool(b) => Ok(b),
        v => error(format!(
            "Expected a boolean result, got a value of type `{}`",
            v.value_type()
        )),
    }
}

#[signature(
    stream.any_match,
    can_block = true,
    output = Known(ValueType::Bool),
    short = "True if `condition` is true for at least one row of input.",
    long = "Stops reading input as soon as one matching row is found, rather than always",
    long = "consuming the whole stream. The columns of the row are exported to `condition`",
    long = "by name, exactly like `where`.",
    example = "# Is there any process using more than 50% of a CPU core?",
    example = "host:procs | any_match {($cpu > 50)}",
)]
pub struct AnyMatch {
    #[description("the condition to check for each row.")]
    condition: Command,
}

fn any_match(mut context: CommandContext) -> CrushResult<()> {
    let source = context.arguments[0].source.clone();
    let cfg = AnyMatch::parse(context.remove_arguments(), &context.global_state.printer())?;

    let mut input = context.input_stream()?;
    let base_context = context.empty();

    while let Some(row) = input.next_row()? {
        if evaluate(&cfg.condition, &source, &row, input.types(), &base_context)? {
            return context.output.send(Value::Bool(true));
        }
    }
    context.output.send(Value::Bool(false))
}

#[signature(
    stream.all_match,
    can_block = true,
    output = Known(ValueType::Bool),
    short = "True if `condition` is true for every row of input.",
    long = "Stops reading input as soon as one non-matching row is found, rather than",
    long = "always consuming the whole stream. True for an empty input, same as an empty",
    long = "`and` chain would be. The columns of the row are exported to `condition` by",
    long = "name, exactly like `where`.",
    example = "# Are all processes owned by root?",
    example = "host:procs | all_match {($user == \"root\")}",
)]
pub struct AllMatch {
    #[description("the condition to check for each row.")]
    condition: Command,
}

fn all_match(mut context: CommandContext) -> CrushResult<()> {
    let source = context.arguments[0].source.clone();
    let cfg = AllMatch::parse(context.remove_arguments(), &context.global_state.printer())?;

    let mut input = context.input_stream()?;
    let base_context = context.empty();

    while let Some(row) = input.next_row()? {
        if !evaluate(&cfg.condition, &source, &row, input.types(), &base_context)? {
            return context.output.send(Value::Bool(false));
        }
    }
    context.output.send(Value::Bool(true))
}
