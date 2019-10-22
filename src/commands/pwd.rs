use crate::{
    data::Argument,
    data::Row,
    data::{CellDefinition, CellType},
    stream::{OutputStream, InputStream},
    data::Cell,
    commands::{Call, Exec},
    errors::JobError,
    env::get_cwd
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::CellFnurp;

pub struct Config {output: OutputStream}

pub fn run(
    config: Config,
    _env: Env,
    _printer: Printer,
) -> Result<(), JobError> {
    config.output.send(Row { cells: vec![Cell::File(get_cwd()?)] })?;
    Ok(())
}

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    return Ok((Exec::Pwd(Config {output}), vec![CellFnurp::named("directory", CellType::File)]))
}
