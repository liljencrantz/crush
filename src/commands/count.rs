use crate::{
    errors::{JobError, argument_error},
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellDefinition,
        CellType,
        Cell
    },
    stream::{OutputStream, InputStream},
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::CellFnurp;

pub fn has_streams(input_type: &Vec<CellFnurp>) -> bool {
    for t in input_type.iter() {
        match t.cell_type {
            CellType::Output(_) => return true,
            _ => (),
        }
    }
    return false;
}

fn get_output_type(input_type: &Vec<CellFnurp>) -> Vec<CellDefinition> {
    let res: Vec<CellDefinition> =  input_type.iter().map(|t|
        match t.cell_type {
            CellType::Output(_) => CellFnurp { name: t.name.clone(), cell_type: CellType::Integer},
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

pub fn run(config: Config, env: Env, printer: Printer) -> Result<(), JobError> {
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

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    return Ok(Call {
        name: String::from("group"),
        output_type: if has_streams(&input_type) {
            get_output_type(&input_type)
        } else {
            vec![CellDefinition::named("count", CellType::Integer)]
        },
        input_type,
        arguments,
        exec: Exec::Command(run),
    });
}
