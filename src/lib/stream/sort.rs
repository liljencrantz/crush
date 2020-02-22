use crate::{
    lib::command_util::find_field_from_str,
    errors::argument_error,
    stream::{OutputStream},
};
use crate::lang::ExecutionContext;
use crate::lang::{Argument, Value, Row, RowsReader};
use crate::errors::{CrushResult, error};
use crate::lib::command_util::find_field;
use crate::stream::Readable;
use crate::lib::parse_util::single_argument_field;

pub struct Config<T: Readable> {
    sort_column_idx: usize,
    input: T,
    output: OutputStream,
}

fn parse<T: Readable>(
    arguments: Vec<Argument>,
    input: T,
    output: OutputStream) -> CrushResult<Config<T>> {
    let sort_column_idx = find_field(&single_argument_field(arguments)?, input.types())?;
    if !input.types()[sort_column_idx].cell_type.is_comparable() {
        return argument_error("Bad comparison key");
    }
    Ok(Config { sort_column_idx, input, output })
}

pub fn run<T: Readable>(mut config: Config<T>) -> CrushResult<()> {
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

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Stream(s) => {
            let input = s.stream;
            let output = context.output.initialize(input.types().clone())?;
            let config = parse(context.arguments, input, output)?;
            run(config)
        }
        Value::Rows(r) => {
            let input = RowsReader::new(r);
            let output = context.output.initialize(input.types().clone())?;
            let mut config = parse(context.arguments, input, output)?;
            run(config)
        }
        _ => error("Expected a stream"),
    }
}
