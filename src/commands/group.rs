use crate::commands::CompileContext;
use std::collections::HashMap;
use crate::{
    commands::command_util::find_field,
    errors::{JobError, argument_error},
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellDefinition,
        JobOutput,
        CellType,
        Cell,
    },
    stream::{OutputStream, InputStream, unlimited_streams},
};
use crate::printer::Printer;
use crate::replace::Replace;
use crate::env::Env;
use crate::data::ColumnType;
use crate::errors::JobResult;

pub struct Config {
    input_type: Vec<ColumnType>,
    name: Box<str>,
    column: usize,
}

pub fn parse(input_type: Vec<ColumnType>, arguments: Vec<Argument>) -> JobResult<Config> {
    if arguments.len() != 1 {
        return Err(argument_error("No comparison key specified"));
    }
    let arg = &arguments[0];
    let name = arg.name.clone().unwrap_or(Box::from("group"));
    match &arg.cell {
        Cell::Field(cell_name) | Cell::Text(cell_name) =>
            Ok(Config {
                column: find_field(cell_name, &input_type)?,
                input_type,
                name,
            }),
        _ =>
            Err(argument_error("Bad comparison key")),
    }
}

pub fn run(
    config: Config,
    input: InputStream,
    output: OutputStream,
) -> JobResult<()> {
    let mut groups: HashMap<Cell, OutputStream> = HashMap::new();

    loop {
        match input.recv() {
            Ok(row) => {
                let key = row.cells[config.column].partial_clone()?;
                let val = groups.get(&key);
                match val {
                    None => {
                        let (output_stream, input_stream) = unlimited_streams();
                        let out_row = Row {
                            cells: vec![key.partial_clone()?, Cell::JobOutput(JobOutput { types: config.input_type.clone(), stream: input_stream })],
                        };
                        output.send(out_row)?;
                        output_stream.send(row)?;
                        groups.insert(key, output_stream);
                    }
                    Some(output_stream) => {
                        output_stream.send(row)?;
                    }
                }
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let config = parse(context.input_type.clone(), context.arguments)?;
    let output_type= vec![context.input_type[config.column].clone(), ColumnType { name: Some(config.name.clone()), cell_type: CellType::Output(context.input_type.clone()) }];
    let input = context.input;
    let output = context.output;
    Ok((Exec::Command(Box::from(move || run(config, input, output))), output_type))
}
