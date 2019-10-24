use crate::{
    data::Argument,
    data::Row,
    data::{CellType},
    stream::{OutputStream, InputStream},
    data::Cell,
    commands::{Exec},
    errors::JobError,
    env::get_cwd
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::ColumnType;

pub struct Config {output: OutputStream}

pub fn run(
    config: Config,
    _env: Env,
    _printer: Printer,
) -> Result<(), JobError> {
    config.output.send(Row { cells: vec![Cell::File(get_cwd()?)] })?;
    Ok(())
}

pub fn compile(_input_type: Vec<ColumnType>, _input: InputStream, output: OutputStream, _arguments: Vec<Argument>) -> Result<(Exec, Vec<ColumnType>), JobError> {
    return Ok((Exec::Pwd(Config {output}), vec![ColumnType::named("directory", CellType::File)]))
}
