use crate::stream::{OutputStream, InputStream};
use crate::data::{Argument, CellType, Row, CellDataType};
use crate::commands::{Call, Exec};
use crate::errors::{JobError, argument_error};
use std::iter::Iterator;
use crate::data::cell::Cell;

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream) -> Result<(), JobError> {
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
    let mut output_type = vec![CellType { name: "idx".to_string(), cell_type: CellDataType::Integer }];
    output_type.extend(input_type.iter().cloned());
    return Ok(Call {
        name: String::from("enumerate"),
        output_type,
        input_type,
        arguments,
        exec: Exec::Run(run),
    });
}
