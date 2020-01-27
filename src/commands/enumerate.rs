use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::data::ValueType;
use crate::data::Row;
use crate::data::Value;
use crate::stream::OutputStream;
use crate::stream::InputStream;
use crate::data::ColumnType;

pub fn run(input: InputStream, output: OutputStream) -> JobResult<()> {
    let mut line: i128 = 1;
    loop {
        match input.recv() {
            Ok(row) => {
                let mut out = vec![Value::Integer(line)];
                out.extend(row.cells);
                output.send(Row { cells: out })?;
                line += 1;
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn perform(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize_stream()?;
    let mut output_type = vec![ColumnType::named("idx", ValueType::Integer)];
    output_type.extend(input.get_type().clone());
    let output = context.output.initialize(output_type)?;
    run(input, output)
}
