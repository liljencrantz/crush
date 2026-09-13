use crate::lang::ast::source::Source;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::CrushResult;
use crate::lang::pipe::pipe;
use crate::lang::state::contexts::CommandContext;
use crate::lang::{argument::Argument, data::table::ColumnType};
use crate::lang::{data::table::Row, value::Value};
use signature::signature;

#[signature(
    stream.fold,
    can_block = true,
    output = Unknown,
    short = "Accumulate a single result across all rows of input using a user-supplied closure.",
    long = "`body` is called once per input row, with the accumulator available as `acc`",
    long = "and the row's own columns exported by name, exactly like `where`/`each`. Its",
    long = "return value becomes the accumulator for the next row; once the input is",
    long = "exhausted, the final accumulator is fold's own output. This is the general",
    long = "escape hatch for one-off aggregations that don't have (or don't need) their own",
    long = "dedicated command like `sum` or `max`.",
    example = "# Sum 1 through 5 by hand",
    example = "seq 1 6 | fold {($acc + $value)} initial=0",
    example = "# Build a comma separated string from a column",
    example = "files | fold {(\"{}, {}\":format($acc, $file))} initial=\"\"",
)]
pub struct Fold {
    #[description("the accumulator's value before the first row is processed.")]
    initial: Value,
    #[description(
        "called once per row with the accumulator (`acc`) and the row's own columns as named arguments; its return value is the next accumulator."
    )]
    body: Command,
}

fn accumulate(
    body: Command,
    source: &Source,
    acc: Value,
    row: &Row,
    input_type: &[ColumnType],
    base_context: &CommandContext,
) -> CrushResult<Value> {
    let mut arguments: Vec<Argument> = Vec::from(row.clone())
        .drain(..)
        .zip(input_type.iter())
        .map(|(c, t)| Argument::named(t.name(), c, source))
        .collect();
    arguments.push(Argument::named("acc", acc, source));

    let (sender, receiver) = pipe();

    body.eval(
        base_context
            .clone()
            .with_args(arguments, None)
            .with_output(sender),
    )?;

    receiver.recv()
}

fn fold(mut context: CommandContext) -> CrushResult<()> {
    let source = context.arguments[0].source.clone();
    let cfg = Fold::parse(context.remove_arguments(), &context.global_state.printer())?;

    let mut input = context.input_stream()?;
    let base_context = context.empty();

    let mut acc = cfg.initial;
    while let Some(row) = input.next_row()? {
        acc = accumulate(
            cfg.body.clone(),
            &source,
            acc,
            &row,
            input.types(),
            &base_context,
        )?;
    }
    context.output.send(acc)
}
