use std::collections::HashMap;
use crate::{
    commands::command_util::find_field,
    errors::{JobError, argument_error},
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellType,
        Output,
        CellDataType,
        Cell
    },
    stream::{OutputStream, InputStream, unlimited_streams},
};

pub fn get_key(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<(Option<String>, usize), JobError> {
    if arguments.len() != 1 {
        return Err(argument_error("No comparison key specified"));
    }
    let arg = &arguments[0];
    let name = arg.name.as_ref().unwrap_or(&"group".to_string()).clone();
    match &arg.cell {
        Cell::Field(cell_name) | Cell::Text(cell_name) => {
            return Ok((Some(name), find_field(cell_name, &input_type)?));
        }
        _ => {
            return Err(argument_error("Bad comparison key"));
        }
    }
}

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream) -> Result<(), JobError> {
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
                            cells: vec![key.partial_clone()?, Cell::Output(Output { types: input_type.clone(), stream: input_stream })],
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

pub fn group(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    let (column_name, column) = get_key(&input_type, &arguments)?;
    return Ok(Call {
        name: String::from("group"),
        output_type: vec![input_type[column].clone(), CellType { name: column_name, cell_type: CellDataType::Output(input_type.clone()) }],
        input_type,
        arguments,
        exec: Exec::Run(run),
    });
}
