use crate::lang::command::ExecutionContext;
use std::collections::HashMap;
use crate::{
    lib::command_util::find_field_from_str,
    lang::errors::{argument_error},
    lang::{
        argument::Argument,
        table::Row,
        value::ValueType,
        value::Value,
    },
    lang::stream::{OutputStream, unlimited_streams},
};
use crate::lang::{table::ColumnType, table::TableReader};
use crate::lang::errors::{CrushResult, error};
use crate::lib::command_util::find_field;
use crate::lang::stream::{Readable};

pub struct Config {
    input_type: Vec<ColumnType>,
    name: Box<str>,
    column: usize,
}

pub fn parse(input_type: Vec<ColumnType>, arguments: Vec<Argument>) -> CrushResult<Config> {
    if arguments.len() != 1 {
        return argument_error("No comparison key specified");
    }
    let arg = &arguments[0];
    let name = arg.argument_type.clone().unwrap_or(Box::from("group"));
    match &arg.value {
        Value::String(cell_name) =>
            Ok(Config {
                column: find_field_from_str(cell_name, &input_type)?,
                input_type,
                name,
            }),
        Value::Field(cell_name) =>
            Ok(Config {
                column: find_field(cell_name, &input_type)?,
                input_type,
                name,
            }),
        _ => argument_error("Bad comparison key"),
    }
}

pub fn run(
    config: Config,
    input: &mut dyn Readable,
    output: OutputStream,
) -> CrushResult<()> {
    let mut groups: HashMap<Value, OutputStream> = HashMap::new();

    loop {
        match input.read() {
            Ok(row) => {
                let key = row.cells()[config.column].clone();
                let val = groups.get(&key);
                match val {
                    None => {
                        let (output_stream, input_stream) = unlimited_streams(config.input_type.clone());
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
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(mut input) => {
            let config = parse(input.types().clone(), context.arguments)?;
            let output_type= vec![
                input.types()[config.column].clone(),
                ColumnType::new(
                    &config.name,
                    ValueType::TableStream(input.types().clone()))
            ];
            let output = context.output.initialize(output_type)?;
            run(config, input.as_mut(), output)
        }
        None => error("Expected a stream"),
    }
}
