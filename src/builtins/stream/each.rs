use crate::lang::ast::source::Source;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ValueType::Empty;
use crate::lang::{argument::Argument, data::table::ColumnType};
use crate::lang::{data::table::Row, value::Value};
use signature::signature;

#[signature(
    stream.r#each,
    can_block = true,
    output = Known(Empty),
    short = "Runs a command one for each row of input",
    long = "The columns of the row are exported to the environment using the column names.",
    example = "host:procs | where {$status != \"Sleeping\"} | each {echo (\"{} is sleepy\":format $name)}"
)]
pub struct Each {
    #[description("the command to run.")]
    body: Command,
}

fn run(
    condition: &Command,
    source: &Source,
    row: &Row,
    input_type: &[ColumnType],
    base_context: &CommandContext,
) -> CrushResult<()> {
    let arguments = Vec::from(row.clone())
        .drain(..)
        .zip(input_type.iter())
        .map(|(c, t)| Argument::named(t.name(), c, source))
        .collect();

    condition.eval(base_context.clone().with_args(arguments, None))
}

pub fn each(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Each::parse(
        context.remove_arguments().clone(),
        &context.global_state.printer(),
    )?;
    let source = &context.arguments[0].source;
    context.output.send(Value::Empty)?;

    let mut input = context.input.recv()?.stream()?;
    let base_context = context.empty();

    while let Ok(row) = input.read() {
        match run(&cfg.body, source, &row, input.types(), &base_context) {
            Ok(_) => (),
            Err(e) => base_context.global_state.printer().crush_error(e),
        }
    }
    Ok(())
}
