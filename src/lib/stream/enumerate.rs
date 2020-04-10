use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::{CrushResult, error};
use crate::lang::{value::ValueType, table::Row, value::Value};
use crate::lang::stream::{Readable, ValueSender};
use crate::lang::table::ColumnType;

pub fn run(input: &mut dyn Readable, sender: ValueSender) -> CrushResult<()> {
    let mut output_type = vec![ColumnType::new("idx", ValueType::Integer)];
    output_type.extend(input.types().clone());
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

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(mut r) => run(r.as_mut(), context.output),
        None => error("Expected a stream"),
    }
}
