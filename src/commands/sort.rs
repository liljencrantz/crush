use crate::{
    commands::command_util::find_field_from_str,
    errors::argument_error,
    stream::{InputStream, OutputStream},
};
use crate::commands::CompileContext;
use crate::data::{Argument, Value, Row};
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
    let sort_column_idx = match (&arguments[0].name, &arguments[0].value) {
        (None, Value::Text(cell_name)) => find_field_from_str(cell_name, input.get_type())?,
        (None, Value::Field(cell_name)) => find_field(cell_name, input.get_type())?,
        _ => return Err(argument_error("No comparison key specified")),
    };
    if !input.get_type()[sort_column_idx].cell_type.is_comparable() {
        return Err(argument_error("Bad comparison key"));
    }
    Ok(Config { sort_column_idx, input, output })
}

pub fn run(config: Config) -> JobResult<()> {
    let mut res: Vec<Row> = Vec::new();
    loop {
        match config.input.recv() {
            Ok(row) => res.push(row),
            Err(_) => break,
        }
    }

    res.sort_by(|a, b|
        a.cells[config.sort_column_idx]
            .partial_cmp(&b.cells[config.sort_column_idx])
            .expect("OH NO!"));

    for row in res {
        config.output.send(row)?;
    }

    return Ok(());
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize_stream()?;
    let output = context.output.initialize(input.get_type().clone())?;
    let config = parse(context.arguments, input, output)?;
    run(config)
}
