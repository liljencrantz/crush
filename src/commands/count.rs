use crate::{
    errors::{JobError, argument_error},
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellType,
        CellDataType,
        Cell
    },
    stream::{OutputStream, InputStream},
};
use crate::printer::Printer;
use crate::env::Env;

pub fn has_streams(input_type: &Vec<CellType>) -> bool {
    for t in input_type.iter() {
        match t.cell_type {
            CellDataType::Output(_) => return true,
            _ => (),
        }
    }
    return false;
}

fn get_output_type(input_type: &Vec<CellType>) -> Vec<CellType> {
    let res: Vec<CellType> =  input_type.iter().map(|t|
        match t.cell_type {
            CellDataType::Output(_) => CellType{ name: t.name.clone(), cell_type: CellDataType::Integer},
            _ => t.clone(),
        }).collect();
    return res;
}

fn count_rows(s: &InputStream) -> Cell {
    let mut res: i128 = 0;
    loop {
        match s.recv() {
            Ok(_) => res+= 1,
            Err(_) => break,
        }
    }
    return Cell::Integer(res);
}

fn run(
    input_type: Vec<CellType>,
    _arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    if has_streams(&input_type) {
        loop {
            match input.recv() {
                Ok(row) => {
                    let mut cells: Vec<Cell> = Vec::new();
                    for c in row.cells {
                        match &c {
                            Cell::Output(o) => cells.push(count_rows(&o.stream)),
                            _ => {
                                cells.push(c)
                            }
                        }
                    }
                    output.send(Row { cells })?;
                }
                Err(_) => break,
            }
        }
    } else {
        output.send(Row { cells: vec![count_rows(&input)]})?;
    }
    return Ok(());
}

pub fn count(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    return Ok(Call {
        name: String::from("group"),
        output_type: if has_streams(&input_type) {
            get_output_type(&input_type)
        } else {
            vec![CellType::named("count", CellDataType::Integer)]
        },
        input_type,
        arguments,
        exec: Exec::Command(run),
    });
}
