use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::{
    data::{
        CellDefinition,
        CellType,
        Row,
        Argument,
        Cell,
    },
    stream::{OutputStream, InputStream},
    commands::{Call, Exec},
    errors::{JobError, argument_error},
};
use std::iter::Iterator;
use crate::printer::Printer;
use crate::env::Env;
use crate::data::ColumnType;


pub fn run(
    input: InputStream,
    output: OutputStream,
) -> JobResult<()> {
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

pub fn compile(context: CompileContext) -> JobResult<(Exec, Vec<ColumnType>)> {
    let mut output_type = vec![ColumnType::named("idx", CellType::Integer)];
    let input = context.input;
    let output = context.output;
    output_type.extend(context.input_type.iter().cloned());
    return Ok((Exec::Command(Box::from(move|| run(input, output))), output_type));
}
