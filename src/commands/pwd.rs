use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::{
    data::Argument,
    data::Row,
    data::{CellType},
    stream::{OutputStream, InputStream},
    data::Cell,
    errors::JobError,
    env::get_cwd
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::ColumnType;

pub fn run(output: OutputStream) -> JobResult<()> {
    output.send(Row { cells: vec![Cell::File(get_cwd()?)] })
}

pub fn parse_and_run(context: CompileContext) -> JobResult<()> {
    run(context.output.initialize(vec![ColumnType::named("directory", CellType::File)])?)
}
