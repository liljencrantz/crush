use crate::{
    errors::JobError,
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellType,
        CellDataType,
        Output,
        Cell
    },
    stream::{OutputStream, InputStream},
};
use std::collections::HashMap;

struct Config {
    left_idx: usize,
    right_idx: usize,
}

    fn parse(_input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Config, JobError> {
    Ok(Config { left_idx: 0, right_idx: 0 })
}

fn do_join(l: &Output, r: &Output, output: &OutputStream) {

    let mut l_data: HashMap<Cell, Row> = HashMap::new();
    loop {
        match l.stream.recv() {
            Ok(row) => {
                l_data.insert(row.cells[0].concrete(), row);
            }
            Err(_) => break,
        }
    }

    loop {
        match r.stream.recv() {
            Ok(r_row) => {
                l_data.get(&r_row.cells[0].concrete()).map(|l_row| {
                    output.send(Row{ cells: vec![
                        r_row.cells[0].concrete(),
                        l_row.cells[1].concrete(),
                        r_row.cells[1].concrete(),
                    ] });
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
    let cfg = parse(input_type, arguments)?;

    loop {
        match input.recv() {
            Ok(row) => {

                match (&row.cells[0], &row.cells[1]) {
                    (Cell::Output(l), Cell::Output(r)) => {
                        do_join(l, r, &output);
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
    let output_type = vec![
        CellType { name: "name".to_string(), cell_type: CellDataType::Text },
        CellType { name: "age".to_string(), cell_type: CellDataType::Integer },
        CellType { name: "home".to_string(), cell_type: CellDataType::Text },
    ];
    return Ok(Call {
        name: String::from("join"),
        input_type,
        arguments,
        output_type,
        exec: Exec::Run(run),
    });
}
