use crate::{
    errors::JobError,
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellType,
        CellDataType,
        Cell,
    },
    stream::{OutputStream, InputStream},
};
use std::collections::HashMap;
use crate::stream::Readable;
use crate::replace::Replace;

struct Config {
    left_table_idx: usize,
    right_table_idx: usize,
    left_column_idx: usize,
    right_column_idx: usize,
}

fn parse(_input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Config, JobError> {
    Ok(Config {
        left_table_idx: 0,
        right_table_idx: 2,
        left_column_idx: 0,
        right_column_idx: 0
    })
}

fn do_join(l: &mut impl Readable, r: &mut impl Readable, output: &OutputStream) {
    let mut l_data: HashMap<Cell, Row> = HashMap::new();
    loop {
        match l.read() {
            Ok(row) => {
                l_data.insert(row.cells[0].concrete(), row);
            }
            Err(_) => break,
        }
    }

    loop {
        match r.read() {
            Ok(r_row) => {
                l_data.get(&r_row.cells[0].concrete()).map(|l_row| {
                    output.send(Row {
                        cells: vec![
                            r_row.cells[0].concrete(),
                            l_row.cells[1].concrete(),
                            r_row.cells[1].concrete(),
                        ]
                    });
                }
                );
            }
            Err(_) => break,
        }
    }
}

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream) -> Result<(), JobError> {
    let cfg = parse(&input_type, &arguments)?;

    loop {
        match input.recv() {
            Ok(mut row) => {
                match (row.cells.replace(cfg.left_table_idx, Cell::Integer(0)), row.cells.replace(cfg.right_table_idx, Cell::Integer(0))) {
                    (Cell::Output(mut l), Cell::Output(mut r)) => {
                        do_join(&mut l.stream, &mut r.stream, &output);
                    }
                    _ => panic!("Wrong row format"),
                }
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn join(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    let cfg = parse(&input_type, &arguments);
    let output_type = vec![
        CellType::named("name", CellDataType::Text ),
        CellType::named("age", CellDataType::Integer ),
        CellType::named("home", CellDataType::Text ),
    ];
    return Ok(Call {
        name: String::from("join"),
        input_type,
        arguments,
        output_type,
        exec: Exec::Run(run),
    });
}
