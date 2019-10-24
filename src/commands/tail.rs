use crate::{
    data::Row,
    data::{CellDefinition},
    stream::{OutputStream, InputStream},
    data::Argument,
    commands::{Call, Exec},
    errors::{JobError, argument_error},
    commands::head::get_line_count
};
use std::collections::VecDeque;
use crate::printer::Printer;
use crate::env::Env;
use crate::data::ColumnType;
use crate::commands::head;

pub type Config = head::Config;

pub fn run(
    config: Config,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    let mut q: VecDeque<Row> = VecDeque::new();
    loop {
        match config.input.recv() {
            Ok(row) => {
                if q.len() >= config.lines as usize {
                    q.pop_front();
                }
                q.push_back(row);
            }
            Err(_) => {
                for row in q.drain(..) {
                    config.output.send(row)?;
                }
                break;
            },
        }
    }
    return Ok(());
}

pub fn compile(input_type: Vec<ColumnType>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<ColumnType>), JobError> {
    Ok((Exec::Tail(Config {
        lines: get_line_count(arguments)?,
        input,
        output
    }), input_type))
}
