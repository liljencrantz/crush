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

fn run(
    _input_type: Vec<CellType>,
    _arguments: Vec<Argument>,
    _input: InputStream,
    output: OutputStream) -> Result<(), JobError> {

    output.send(Row { cells: vec![Cell::File(get_cwd()?)] })?;
    Ok(())
}

pub(crate) fn pwd(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    return Ok(Call {
        name: String::from("pwd"),
        input_type,
        arguments,
        output_type: vec![CellType::named("directory", CellDataType::File)],
        exec: Exec::Run(run),
    });
}
