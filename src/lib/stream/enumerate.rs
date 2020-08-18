use crate::lang::errors::{error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::stream::{CrushStream, ValueSender};
use crate::lang::table::ColumnType;
use crate::lang::{table::Row, value::Value, value::ValueType};

pub fn run(input: &mut dyn CrushStream, sender: ValueSender) -> CrushResult<()> {
    let mut output_type = vec![ColumnType::new("idx", ValueType::Integer)];
    output_type.extend(input.types().to_vec());
    let output = sender.initialize(output_type)?;

    let mut line: i128 = 0;
    while let Ok(row) = input.read() {
        let mut out = vec![Value::Integer(line)];
        out.extend(row.into_vec());
        output.send(Row::new(out))?;
        line += 1;
    }
    Ok(())
}

pub fn perform(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream() {
        Some(mut r) => run(r.as_mut(), context.output),
        None => error("Expected a stream"),
    }
}
