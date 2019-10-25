use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::{
    stream::{OutputStream, InputStream},
    data::Row,
    data::Argument,
    commands::{Exec},
    errors::JobError
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::ColumnType;

pub struct Config {
    arguments: Vec<Argument>,
    output: OutputStream,
}

pub fn run(mut config: Config) -> JobResult<()> {
    config.output.send(Row {
        cells: config.arguments.drain(..).map(|c| c.cell).collect()
    })
}

pub fn compile(context: CompileContext) -> JobResult<(Exec, Vec<ColumnType>)> {
    let output_type = context.arguments.iter().map(Argument::cell_type).collect();
    let config = Config{arguments: context.arguments, output: context.output};
    Ok((Exec::Command(Box::from(move || run(config))), output_type))
}
