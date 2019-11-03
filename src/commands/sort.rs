use crate::{
    commands::command_util::find_field_from_str,
    errors::argument_error,
    stream::{InputStream, OutputStream},
};
use crate::commands::CompileContext;
use crate::data::{Argument, Cell, Row};
use crate::errors::JobResult;
use crate::commands::command_util::find_field;

pub struct Config {
    sort_column_idx: usize,
    input: InputStream,
    output: OutputStream,
}

fn parse(
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream) -> JobResult<Config> {
    if arguments.len() != 1 {
        return Err(argument_error("No comparison key specified"));
    }
    match (&arguments[0].name, &arguments[0].cell) {
        (None, Cell::Text(cell_name)) => {
            Ok(Config {
                sort_column_idx: find_field_from_str(cell_name, input.get_type())?,
                input,
                output,
            })
        }
        (None, Cell::Field(cell_name)) => {
            Ok(Config {
                sort_column_idx: find_field(cell_name, input.get_type())?,
                input,
                output,
            })
        }
        _ => Err(argument_error("Bad comparison key"))
    }
}

pub fn run(config: Config) -> JobResult<()> {
    let mut res: Vec<Row> = Vec::new();
    loop {
        match config.input.recv() {
            Ok(row) => res.push(row),
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

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize()?;
    let output = context.output.initialize(input.get_type().clone())?;
    let config = parse(context.arguments, input, output)?;
    run(config)
}
