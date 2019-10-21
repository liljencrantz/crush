use crate::{
    data::{
        CellType,
        CellDataType,
        Row,
        Argument,
        Cell
    },
    stream::{OutputStream, InputStream},
    commands::{Call, Exec},
    errors::{JobError, argument_error},
};
use std::iter::Iterator;
use crate::printer::Printer;
use crate::env::Env;

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
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

pub fn enumerate(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    let mut output_type = vec![CellType::named("idx", CellDataType::Integer)];
    output_type.extend(input_type.iter().cloned());
    return Ok(Call {
        name: String::from("enumerate"),
        output_type,
        input_type,
        arguments,
        exec: Exec::Command(run),
    });
}
