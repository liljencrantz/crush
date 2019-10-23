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

pub struct Config {
    has_streams: bool,
    input: InputStream,
    output: OutputStream,
}

pub fn parse(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream) -> Config {
    for t in input_type.iter() {
        match t.cell_type {
            CellType::Output(_) => return Config {has_streams: true, input, output},
            CellType::Rows(_) => return Config {has_streams: true, input, output},
            _ => (),
        }
    }
    Config {has_streams: false, input, output}
}

fn get_output_type(input_type: &Vec<CellFnurp>) -> Vec<CellFnurp> {
    let res: Vec<CellFnurp> =  input_type.iter().map(|t|
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
    if config.has_streams {
        loop {
            match config.input.recv() {
                Ok(row) => {
                    let mut cells: Vec<Cell> = Vec::new();
                    for c in row.cells {
                        match &c {
                            Cell::JobOutput(o) => cells.push(count_rows(&o.stream)),
                            Cell::Rows(r) => cells.push(Cell::Integer(r.rows.len() as i128)),
                            _ => {
                                cells.push(c)
                            }
                        }
                    }
                    config.output.send(Row { cells })?;
                }
                Err(_) => break,
            }
        }
    } else {
        config.output.send(Row { cells: vec![count_rows(&config.input)]})?;
    }
    return Ok(());
}

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    let config = parse(input_type.clone(), input, output);
    let output_type = if config.has_streams {
        get_output_type(&input_type)
    } else {
        vec![CellFnurp::named("count", CellType::Integer)]
    };

    Ok((Exec::Count(config), output_type))
}
