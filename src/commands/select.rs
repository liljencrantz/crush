use std::iter::Iterator;
use crate::{
    commands::command_util::find_field,
    errors::{JobError, argument_error},
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellType,
        Cell
    },
    stream::{OutputStream, InputStream},
};
use crate::replace::Replace;

fn parse(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Vec<(usize, Option<String>)>, JobError> {
    arguments.iter().enumerate().map(|(idx, a)| {
        match &a.cell {
            Cell::Text(s) | Cell::Field(s) => match find_field(s, input_type) {
                Ok(idx) => Ok((idx, a.name.clone().or(input_type[idx].name.clone()))),
                Err(e) => Err(e),
            }
            _ => Err(argument_error(format!("Expected Field, not {:?}", a.cell.cell_data_type()).as_str())),
        }
    }).collect()
}

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream) -> Result<(), JobError> {
    let indices = parse(&input_type, &arguments)?;
    loop {
        match input.recv() {
            Ok(mut row) => {
                output.send(
                    Row { cells: indices
                        .iter()
                        .map(|(idx, name)| row.cells.replace(*idx, Cell::Integer(0)))
                        .collect() })?;
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn select(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    let mut indices = parse(&input_type, &arguments)?;
    return Ok(Call {
        name: String::from("select"),
        output_type: indices.drain(..).map(|(idx, name)| CellType {cell_type: input_type[idx].cell_type.clone(), name }).collect(),
        input_type,
        arguments,
        exec: Exec::Run(run),
    });
}
