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
        Cell
    },
    stream::{OutputStream, InputStream, unlimited_streams},
};
use crate::printer::Printer;
use crate::replace::Replace;
use crate::env::Env;
use crate::data::CellFnurp;

pub fn get_key(input_type: &Vec<CellFnurp>, arguments: &Vec<Argument>) -> Result<(Option<Box<str>>, usize), JobError> {
    if arguments.len() != 1 {
        return Err(argument_error("No comparison key specified"));
    }
    let arg = &arguments[0];
    let name = arg.name.clone().unwrap_or(Box::from("group"));
    match &arg.cell {
        Cell::Field(cell_name) | Cell::Text(cell_name) => {
            return Ok((Some(name), find_field(cell_name, &input_type)?));
        }
        _ => {
            return Err(argument_error("Bad comparison key"));
        }
    }
}

pub fn run(
    config: Config,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    let (name, column) = get_key(&input_type, &arguments)?;

    let mut groups: HashMap<Cell, OutputStream> = HashMap::new();

    loop {
        match input.recv() {
            Ok(row) => {
                let key = row.cells[column].partial_clone()?;
                let val = groups.get(&key);
                match val {
                    None => {
                        let (output_stream, input_stream) = unlimited_streams();
                        let out_row = Row {
                            cells: vec![key.partial_clone()?, Cell::JobOutput(JobOutput { types: input_type.clone(), stream: input_stream })],
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

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    let (column_name, column) = get_key(&input_type, &arguments)?;
    return Ok(Call {
        name: String::from("group"),
        output_type: vec![input_type[column].clone(), CellFnurp { name: column_name, cell_type: CellType::Output(input_type.clone()) }],
        input_type,
        arguments,
        exec: Exec::Command(run),
    });
}
