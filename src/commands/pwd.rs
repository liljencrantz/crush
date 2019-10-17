use crate::{
    data::Argument,
    data::Row,
    data::{CellType, CellDataType},
    stream::{OutputStream, InputStream},
    data::Cell,
    commands::{Call, Exec},
    errors::JobError,
    state::get_cwd
};
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
            name: Some(String::from("directory")),
            cell_type: CellDataType::File,
        }],
        exec: Exec::Run(run),
    });
}
