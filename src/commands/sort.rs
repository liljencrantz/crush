use crate::{
    commands::command_util::find_field_from_str,
    errors::argument_error,
    stream::{OutputStream},
};
use crate::commands::CompileContext;
use crate::data::{Argument, Value, Row, RowsReader};
use crate::errors::{JobResult, error};
use crate::commands::command_util::find_field;
use crate::stream::Readable;

pub struct Config<T: Readable> {
    sort_column_idx: usize,
    input: T,
    output: OutputStream,
}

fn parse<T: Readable>(
    arguments: Vec<Argument>,
    input: T,
    output: OutputStream) -> JobResult<Config<T>> {
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

pub fn run<T: Readable>(mut config: Config<T>) -> JobResult<()> {
    let mut res: Vec<Row> = Vec::new();
    loop {
        match config.input.read() {
            Ok(row) => res.push(row),
            Err(_) => break,
        }
    }

    res.sort_by(|a, b|
        a.cells()[config.sort_column_idx]
            .partial_cmp(&b.cells()[config.sort_column_idx])
            .expect("OH NO!"));

    for row in res {
        config.output.send(row)?;
    }

    return Ok(());
}

pub fn perform(context: CompileContext) -> JobResult<()> {
    match context.input.recv()? {
        Value::Stream(s) => {
            let input = s.stream;
            let output = context.output.initialize(input.get_type().clone())?;
            let config = parse(context.arguments, input, output)?;
            run(config)
        }
        Value::Rows(r) => {
            let input = RowsReader::new(r);
            let output = context.output.initialize(input.get_type().clone())?;
            let mut config = parse(context.arguments, input, output)?;
            run(config)
        }
        _ => Err(error("Expected a stream")),
    }
}
