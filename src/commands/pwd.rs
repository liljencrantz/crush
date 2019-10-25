use crate::commands::CompileContext;
use crate::errors::JobResult;
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

pub fn run(
    output: OutputStream,
) -> JobResult<()> {
    output.send(Row { cells: vec![Cell::File(get_cwd()?)] })?;
    Ok(())
}

pub fn compile(context: CompileContext) -> JobResult<(Exec, Vec<ColumnType>)> {
    return Ok((Exec::Command(Box::from(move || run(context.output))), vec![ColumnType::named("directory", CellType::File)]))
}
