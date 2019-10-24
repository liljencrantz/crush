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

pub fn run(mut config: Config, env: Env, printer: Printer) -> Result<(), JobError> {
    config.output.send(Row {
        cells: config.arguments.drain(..).map(|c| c.cell).collect()
    })
}

pub fn compile(input_type: Vec<ColumnType>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<ColumnType>), JobError> {
    let output_type = arguments.iter().map(Argument::cell_type).collect();
    Ok((Exec::Echo(Config{arguments, output}), output_type))
}
