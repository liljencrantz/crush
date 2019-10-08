use crate::stream::{OutputStream, InputStream};
use crate::cell::{Argument, CellType, Row};
use crate::commands::{Call};
use crate::errors::JobError;
use crate::state::State;

fn run(
    _input_type: &Vec<CellType>,
    arguments: &Vec<Argument>,
    _input: &mut InputStream,
    output: &mut OutputStream) -> Result<(), JobError> {
    let g = arguments.iter().map(|c| c.cell.clone());
    output.send(Row {
        cells: g.collect()
    });
    return Ok(());
}

pub fn echo(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Call, JobError> {
    let output_type = arguments
        .iter()
        .map(|a| CellType { name: a.name.clone(), cell_type: a.cell.cell_data_type() })
        .collect();
    return Ok(Call {
        name: String::from("echo"),
        input_type: input_type.clone(),
        arguments: arguments.clone(),
        output_type,
        run: Some(run),
        mutate: None,
    });
}
