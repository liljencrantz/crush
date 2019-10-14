use crate::stream::{OutputStream, InputStream, unlimited_streams};
use crate::cell::{Argument, CellType, Cell, Row, Output, CellDataType};
use crate::commands::{Call, Exec};
use crate::errors::{JobError, argument_error};
use std::collections::HashMap;

fn get_group_key(arguments: &Vec<Argument>) -> Result<usize, JobError> {
    return Ok(1usize);
}

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream) -> Result<(), JobError> {

    let column = get_group_key(&arguments)?;

    let mut groups: HashMap<Cell, OutputStream> = HashMap::new();

    loop {
        match input.recv() {
            Ok(row) => {
                let key = row.cells[column].partial_clone()?;
                let val =  groups.get(&key);
                match val {
                    None => {
                        let (output_stream, input_stream) = unlimited_streams();
                        let out_row = Row {
                            cells: vec![key.partial_clone()?, Cell::Output(Output {types:input_type.clone(), stream: input_stream})],
                        };
                        output.send(out_row)?;
                        output_stream.send(row)?;
                        groups.insert(key, output_stream);
                    },
                    Some(s) => {
                        s.send(row)?;
                    },
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
    let column = get_group_key(&arguments)?;
    return Ok(Call {
        name: String::from("group"),
        output_type: vec![input_type[column].clone(), CellType{ name: "group".to_string(), cell_type: CellDataType::Output }],
        input_type,
        arguments,
        exec: Exec::Run(run),
    });
}
