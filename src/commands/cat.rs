use crate::{
    errors::JobError,
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellType,
        CellDataType,
        Output,
        Cell,
    },
    stream::{OutputStream, InputStream},
};
use crate::errors::{argument_error, error};
use crate::commands::command_util::find_field;
use crate::replace::Replace;
use crate::printer::Printer;
use crate::state::State;

struct Config {
    column: usize,
}

fn parse(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Config, JobError> {
    let indices: Vec<usize> = input_type
        .iter()
        .enumerate()
        .filter(|(i, t)| match t.cell_type.clone() {
            CellDataType::Output(_) | CellDataType::Rows(_) => true,
            _ => false,
        })
        .map(|(i, t)| i)
        .collect();
    return match arguments.len() {
        0 => match indices.len() {
            0 => Err(argument_error("No table-type column found")),
            1 => Ok(Config { column: indices[0] }),
            _ => Err(argument_error("Multiple table-type columns found")),
        },
        1 => match &arguments[0].cell {
            Cell::Field(s) | Cell::Text(s) => {
                let idx = find_field(s, &input_type)?;
                if indices.contains(&idx) { Ok(Config { column: idx }) } else { Err(argument_error("Field is not of table-type")) }
            }
            _ => Err(argument_error("Expected a field"))
        },
        _ => Err(argument_error("Expected zero or one arguments"))
    };
}

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
    state: State,
    printer: Printer,
) -> Result<(), JobError> {
    let cfg = parse(&input_type, &arguments)?;
    loop {
        match input.recv() {
            Ok(mut row) => {
                match row.cells.replace(cfg.column, Cell::Integer(0)) {
                    Cell::Output(o) => loop {
                        match o.stream.recv() {
                            Ok(row) => {
                                output.send(row);
                            }
                            Err(_) => break,
                        }
                    }
                    Cell::Rows(mut rows) => {
                        for row in rows.rows {
                            output.send(row);
                        }
                    }
                    _ => return Err(error("Invalid data")),
                }
            }
            Err(_) => break,
        }
    }

    return Ok(());
}

pub fn get_sub_type(cell_type: &CellType) -> Result<Vec<CellType>, JobError> {
    match &cell_type.cell_type {
        CellDataType::Output(o) | CellDataType::Rows(o) => Ok(o.clone()),
        _ => Err(argument_error("Invalid column")),
    }
}

pub fn cat(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    let cfg = parse(&input_type, &arguments)?;
    Ok(Call {
        name: String::from("cat"),
        output_type: get_sub_type(&input_type[cfg.column])?,
        input_type,
        arguments,
        exec: Exec::Command(run),
    })
}
