use crate::lib::ExecutionContext;
use std::collections::HashSet;
use crate::{
    errors::argument_error,
    data::{
        Argument,
        Row,
    },
};
use crate::data::{Value, ColumnType, RowsReader};
use crate::errors::{CrushResult, error};
use crate::lib::command_util::find_field;
use crate::stream::{Readable, OutputStream};

pub struct Config {
    column: Option<usize>,
}

pub fn parse(input_type: &Vec<ColumnType>, arguments: Vec<Argument>) -> CrushResult<Config> {
    match arguments.len() {
        0 => Ok(Config { column: None }),
        1 => match (&arguments[0].name, &arguments[0].value) {
            (None, Value::Field(f)) => Ok(Config { column: Some(find_field(f, input_type)?) }),
            _ => argument_error("Expected field name")
        }
        _ => argument_error("Expected zero or one argument"),
    }
}

pub fn run(
    config: Config,
    mut input: impl Readable,
    output: OutputStream,
) -> CrushResult<()> {
    match config.column {
        None => {
            let mut seen: HashSet<Row> = HashSet::new();
            loop {
                match input.read() {
                    Ok(row) => {
                        if !seen.contains(&row) {
                            seen.insert(row.clone());
                            output.send(row);
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        Some(idx) => {
            let mut seen: HashSet<Value> = HashSet::new();
            loop {
                match input.read() {
                    Ok(row) => {
                        if !seen.contains(&row.cells()[idx]) {
                            seen.insert(row.cells()[idx].clone());
                            output.send(row);
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }
    return Ok(());
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Stream(s) => {
            let input = s.stream;
            let config = parse(input.get_type(), context.arguments)?;
            let output = context.output.initialize(input.get_type().clone())?;
            run(config, input, output)
        }
        Value::Rows(r) => {
            let input = RowsReader::new(r);
            let config = parse(input.get_type(), context.arguments)?;
            let output = context.output.initialize(input.get_type().clone())?;
            run(config, input, output)
        }
        _ => error("Expected a stream"),
    }
}