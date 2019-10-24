use crate::{
    data::Row,
    data::CellDefinition,
    stream::{OutputStream, InputStream},
    data::Argument,
    commands::{Call, Exec},
    errors::{JobError, argument_error},
    commands::head::get_line_count,
};
use std::collections::VecDeque;
use crate::printer::Printer;
use crate::env::Env;
use crate::data::ColumnType;
use crate::commands::head;

pub struct Config {
    pub input: InputStream,
    pub output: OutputStream,
}

pub fn run(
    config: Config,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    let mut q: Vec<Row> = Vec::new();
    loop {
        match config.input.recv() {
            Ok(row) => {
                q.push(row);
            }
            Err(_) => {
                loop {
                    if q.is_empty() {
                        break;
                    }
                    config.output.send(q.pop().unwrap())?;
                }
                break;
            }
        }
    }
    return Ok(());
}

pub fn compile(input_type: Vec<ColumnType>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<ColumnType>), JobError> {
    Ok((Exec::Reverse(Config { input, output }), input_type))
}
