use crate::stream::{OutputStream, InputStream};
use crate::cell::{Argument, CellType, Row};
use crate::commands::{Call, Exec};
use crate::errors::JobError;

fn run(
    _input_type: Vec<CellType>,
    mut arguments: Vec<Argument>,
    _input: InputStream,
    output: OutputStream) -> Result<(), JobError> {
    let g = arguments.drain(..).map(|c| c.cell);
    output.send(Row {
        cells: g.collect()
    })?;
    return Ok(());
}

pub fn echo(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    let output_type = arguments
        .iter()
        .map(|a| CellType { name: a.name.clone(), cell_type: a.cell.cell_data_type() })
        .collect();
    return Ok(Call {
        name: String::from("echo"),
        input_type,
        arguments,
        output_type,
        exec: Exec::Run(run),
    });
}
