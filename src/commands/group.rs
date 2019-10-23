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
use crate::data::CellFnurp;
use crate::errors::JobResult;

pub struct Config {
    input: InputStream,
    output: OutputStream,
    input_type: Vec<CellFnurp>,
    name: Box<str>,
    column: usize,
}

pub fn parse(input_type: Vec<CellFnurp>, arguments: Vec<Argument>, input: InputStream, output: OutputStream) -> JobResult<Config> {
    if arguments.len() != 1 {
        return Err(argument_error("No comparison key specified"));
    }
    let arg = &arguments[0];
    let name = arg.name.clone().unwrap_or(Box::from("group"));
    match &arg.cell {
        Cell::Field(cell_name) | Cell::Text(cell_name) =>
            Ok(Config {
                input: input,
                output: output,
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
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    let mut groups: HashMap<Cell, OutputStream> = HashMap::new();

    loop {
        match config.input.recv() {
            Ok(row) => {
                let key = row.cells[config.column].partial_clone()?;
                let val = groups.get(&key);
                match val {
                    None => {
                        let (output_stream, input_stream) = unlimited_streams();
                        let out_row = Row {
                            cells: vec![key.partial_clone()?, Cell::JobOutput(JobOutput { types: config.input_type.clone(), stream: input_stream })],
                        };
                        config.output.send(out_row)?;
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

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    let config = parse(input_type.clone(), arguments, input, output)?;
    let output_type= vec![input_type[config.column].clone(), CellFnurp { name: Some(config.name.clone()), cell_type: CellType::Output(input_type.clone()) }];
    Ok((Exec::Group(config), output_type))
}
