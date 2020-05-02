use crate::lang::errors::{error, CrushResult};
use crate::lang::execution_context::{ArgumentVector, ExecutionContext};
use crate::lang::stream::Readable;
use crate::lang::table::ColumnType;
use crate::lang::table::ColumnVec;
use crate::{
    lang::errors::argument_error,
    lang::stream::{unlimited_streams, OutputStream},
    lang::{argument::Argument, table::Row, value::Value, value::ValueType},
};
use std::collections::HashMap;

pub struct Config {
    name: String,
    column: usize,
}

pub fn parse(input_type: &[ColumnType], arguments: Vec<Argument>) -> CrushResult<Config> {
    arguments.check_len(1)?;
    let arg = &arguments[0];
    let name = arg
        .argument_type
        .clone()
        .unwrap_or_else(|| "group".to_string());
    match &arg.value {
        Value::String(cell_name) => Ok(Config {
            column: input_type.find_str(cell_name)?,
            name,
        }),
        Value::Field(cell_name) => Ok(Config {
            column: input_type.find(cell_name)?,
            name,
        }),
        _ => argument_error("Bad comparison key"),
    }
}

pub fn run(
    config: Config,
    input_type: &[ColumnType],
    input: &mut dyn Readable,
    output: OutputStream,
) -> CrushResult<()> {
    let mut groups: HashMap<Value, OutputStream> = HashMap::new();

    while let Ok(row) = input.read() {
        let key = row.cells()[config.column].clone();
        let val = groups.get(&key);
        match val {
            None => {
                let (output_stream, input_stream) = unlimited_streams(input_type.to_vec());
                let out_row = Row::new(vec![key.clone(), Value::TableStream(input_stream)]);
                output.send(out_row)?;
                let _ = output_stream.send(row);
                groups.insert(key, output_stream);
            }
            Some(output_stream) => {
                let _ = output_stream.send(row);
            }
        }
    }
    Ok(())
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(mut input) => {
            let config = parse(input.types(), context.arguments)?;
            let output_type = vec![
                input.types()[config.column].clone(),
                ColumnType::new(&config.name, ValueType::TableStream(input.types().to_vec())),
            ];
            let output = context.output.initialize(output_type)?;
            run(config, &input.types().to_vec(), input.as_mut(), output)
        }
        None => error("Expected a stream"),
    }
}
