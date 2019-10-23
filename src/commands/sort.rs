use crate::{
    commands::command_util::find_field,
    errors::{JobError, argument_error},
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellDefinition,
        Cell
    },
    stream::{OutputStream, InputStream},
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::CellFnurp;
use crate::errors::JobResult;

pub struct Config {
    sort_column_idx: usize,
    input: InputStream,
    output: OutputStream,
}

fn parse(
    input_type: Vec<CellFnurp>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream) -> JobResult<Config> {
    if arguments.len() != 1 {
        return Err(argument_error("No comparison key specified"));
    }
    if let Some(name) = &arguments[0].name {
        match (name.as_ref(), &arguments[0].cell) {
            ("key", Cell::Text(cell_name)) | ("key", Cell::Field(cell_name)) => {
                Ok(Config{
                    sort_column_idx: find_field(cell_name, &input_type)?,
                    input,
                    output
                })
            }
            _ => Err(argument_error("Bad comparison key"))
        }
    } else {
        Err(argument_error("Expected comparison key"))
    }
}

pub fn run(
    config: Config,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    let mut res: Vec<Row> = Vec::new();
    loop {
        match config.input.recv() {
            Ok(row) => res.push(row.concrete()),
            Err(_) => break,
        }
    }

    res.sort_by(|a, b| a.cells[config.sort_column_idx]
        .partial_cmp(&b.cells[config.sort_column_idx])
        .expect("OH NO!"));

    for row in res {
        config.output.send(row)?;
    }

    return Ok(());
}

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> JobResult<(Exec, Vec<CellFnurp>)> {
    let output_type = input_type.clone();
    Ok((Exec::Sort(parse(input_type, arguments, input, output)?), output_type))
}
