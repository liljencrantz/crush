use crate::{
    data::{
        CellDefinition,
        CellType,
        Row,
        Argument,
        Cell,
    },
    stream::{OutputStream, InputStream},
    commands::{Call, Exec},
    errors::{JobError, argument_error},
};
use std::iter::Iterator;
use crate::printer::Printer;
use crate::env::Env;
use crate::data::CellFnurp;

pub struct Config {
    input: InputStream,
    output: OutputStream,
}

pub fn run(config: Config, env: Env, printer: Printer) -> Result<(), JobError> {
    let mut line: i128 = 1;
    loop {
        match config.input.recv() {
            Ok(mut row) => {
                let mut out = vec![Cell::Integer(line)];
                out.extend(row.cells);
                config.output.send(Row { cells: out })?;
                line += 1;
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    let mut output_type = vec![CellFnurp::named("idx", CellType::Integer)];
    output_type.extend(input_type.iter().cloned());
    return Ok((Exec::Enumerate(Config {input, output}), output_type))
}
