use crate::stream::{OutputStream, InputStream};
use crate::cell::{Argument, CellType, Cell, Row};
use crate::commands::{Call, Exec};
use crate::errors::{JobError, argument_error};
use crate::commands::filter::find_field;
use crate::commands::group::get_key;


fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream) -> Result<(), JobError> {
    let idx = get_key(&input_type, &arguments)?;
    let mut res: Vec<Row> = Vec::new();
    loop {
        match input.recv() {
            Ok(row) => {
                res.push(row);
            }
            Err(_) => {
                break;
            }
        }
    }
    res.sort_by(|a, b| a.cells[idx].partial_cmp(&b.cells[idx]).expect("OH NO!"));
    for row in res {
        output.send(row)?;
    }

    return Ok(());
}

pub fn sort(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    get_key(&input_type, &arguments)?;
    return Ok(Call {
        name: String::from("Sort"),
        output_type: input_type.clone(),
        input_type,
        arguments,
        exec: Exec::Run(run),
    });
}
