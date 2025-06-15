use crate::lang::data::table::ColumnType;
use crate::lang::errors::{CrushResult, error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::{data::table::Row, value::Value, value::ValueType};
use signature::signature;

#[signature(
    stream.enumerate,
    short = "Prepend a column containing the row number to each row of the input."
)]
pub struct Enumerate {
    #[description("the index to use for the first row.")]
    #[default(0)]
    start_index: i128,
    #[description("the step between rows.")]
    #[default(1)]
    step: i128,
    #[description("the name for the added column.")]
    #[default("idx")]
    name: String,
}

fn enumerate(context: CommandContext) -> CrushResult<()> {
    let cfg = Enumerate::parse(context.arguments, &context.global_state.printer())?;
    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let mut output_type = vec![ColumnType::new_from_string(cfg.name, ValueType::Integer)];
            output_type.extend(input.types().to_vec());
            let output = context.output.initialize(&output_type)?;

            let mut line: i128 = cfg.start_index;
            while let Ok(row) = input.read() {
                let mut out = vec![Value::Integer(line)];
                out.extend(Vec::from(row));
                output.send(Row::new(out))?;
                line += cfg.step;
            }
            Ok(())
        }
        None => error("Expected a stream"),
    }
}
