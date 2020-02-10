use crate::commands::CompileContext;
use crate::errors::{CrushResult, error};
use crate::data::{ValueType, RowsReader};
use crate::data::Row;
use crate::data::Value;
use crate::stream::{OutputStream, Readable};
use crate::data::ColumnType;

pub fn run(mut input: impl Readable, output: OutputStream) -> CrushResult<()> {
    let mut line: i128 = 1;
    loop {
        match input.read() {
            Ok(row) => {
                let mut out = vec![Value::Integer(line)];
                out.extend(row.into_vec());
                output.send(Row::new(out))?;
                line += 1;
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn perform(context: CompileContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Stream(s) => {
            let input = s.stream;
            let mut output_type = vec![ColumnType::named("idx", ValueType::Integer)];
            output_type.extend(input.get_type().clone());
            let output = context.output.initialize(output_type)?;
            run(input, output)
        }
        Value::Rows(r) => {
            let input = RowsReader::new(r);
            let mut output_type = vec![ColumnType::named("idx", ValueType::Integer)];
            output_type.extend(input.get_type().clone());
            let output = context.output.initialize(output_type)?;
            run(input, output)
        }
        _ => Err(error("Expected a stream")),
    }
}
