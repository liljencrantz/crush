use crate::lang::errors::{error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::data::table::ColumnType;
use crate::lang::{data::table::Row, value::Value, value::ValueType};
use signature::signature;

#[signature(enumerate, short = "Prepend a column containing the row number to each row of the input.")]
pub struct Enumerate {
}

fn enumerate(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream() {
        Some(mut input) => {
            let mut output_type = vec![
                ColumnType::new("idx", ValueType::Integer)];
            output_type.extend(input.types().to_vec());
            let output = context.output.initialize(output_type)?;

            let mut line: i128 = 0;
            while let Ok(row) = input.read() {
                let mut out = vec![Value::Integer(line)];
                out.extend(row.into_vec());
                output.send(Row::new(out))?;
                line += 1;
            }
            Ok(())
        }
        None => error("Expected a stream"),
    }
}
