use crate::stream::{OutputStream, InputStream};
use crate::cell::{Argument, CellType, Cell, Row, CellDataType};
use crate::commands::{Call, Exec};
use crate::errors::JobError;
use crate::state::get_cwd;
use std::path::Path;

fn run(
    _input_type: Vec<CellType>,
    _arguments: Vec<Argument>,
    _input: InputStream,
    output: OutputStream) -> Result<(), JobError> {

    output.send(Row {
                        cells: vec![
                            Cell::File(Box::from(Path::new(&get_cwd()?)))
                        ]
                    })?;
    Ok(())
}

pub(crate) fn pwd(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    return Ok(Call {
        name: String::from("pwd"),
        input_type,
        arguments,
        output_type: vec![CellType {
            name: String::from("directory"),
            cell_type: CellDataType::File,
        }],
        exec: Exec::Run(run),
    });
}
