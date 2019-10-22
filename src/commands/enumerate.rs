use crate::{
    data::{
        CellDefinition,
        CellType,
        Row,
        Argument,
        Cell
    },
    stream::{OutputStream, InputStream},
    commands::{Call, Exec},
    errors::{JobError, argument_error},
};
use std::iter::Iterator;
use crate::printer::Printer;
use crate::env::Env;
use crate::data::CellFnurp;

fn run(
    input_type: Vec<CellFnurp>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    let mut line: i128 = 1;
    loop {
        match input.recv() {
            Ok(mut row) => {
                let mut out = vec![Cell::Integer(line)];
                out.extend(row.cells);
                output.send(Row { cells: out })?;
                line += 1;
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    let mut output_type = vec![CellDefinition::named("idx", CellType::Integer)];
    output_type.extend(input_type.iter().cloned());
    return Ok(Call {
        name: String::from("enumerate"),
        output_type,
        input_type,
        arguments,
        exec: Exec::Command(run),
    });
}
