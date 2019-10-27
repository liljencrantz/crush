use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::data::CellType;
use crate::data::Row;
use crate::data::Cell;
use crate::stream::OutputStream;
use crate::stream::InputStream;
use crate::data::ColumnType;

pub fn run(input: InputStream, output: OutputStream) -> JobResult<()> {
    let mut line: i128 = 1;
    loop {
        match input.recv() {
            Ok(mut row) => {
                let mut out = vec![Cell::Integer(line)];
                out.extend(row.cells);
                output.send(Row { cells: out })?;
                line += 1;
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize()?;
    let mut output_type = vec![ColumnType::named("idx", CellType::Integer)];
    output_type.extend(input.get_type().clone());
    let output = context.output.initialize(output_type)?;
    run(input, output)
}
