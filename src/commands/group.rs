use crate::stream::{OutputStream, InputStream, unlimited_streams};
use crate::cell::{Argument, CellType, Cell, Row, Output, CellDataType};
use crate::commands::{Call, Exec};
use crate::errors::{JobError, argument_error};
use std::collections::HashMap;
use crate::commands::filter::find_field;

pub fn get_key(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<usize, JobError> {
    if arguments.len() != 1 {
        return Err(argument_error("No comparison key specified"));
    }
    match (arguments[0].name.as_str(), &arguments[0].cell) {
        ("key", Cell::Text(cell_name)) => {
            return find_field(cell_name, &input_type);
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
    let column = get_key(&input_type, &arguments)?;

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
                    Some(s) => {
                        s.send(row)?;
                    }
                }
            }
            Err(_) => {
                break;
            }
        }
    }
    return Ok(());
}

pub fn group(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    let column = get_key(&input_type, &arguments)?;
    return Ok(Call {
        name: String::from("group"),
        output_type: vec![input_type[column].clone(), CellType { name: "group".to_string(), cell_type: CellDataType::Output }],
        input_type,
        arguments,
        exec: Exec::Run(run),
    });
}
