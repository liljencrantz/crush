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
use crate::errors::argument_error;
use crate::commands::command_util::find_field;

struct Config {
    left_table_idx: usize,
    right_table_idx: usize,
    left_column_idx: usize,
    right_column_idx: usize,
}

pub fn guess_tables(input_type: &Vec<CellType>) -> Result<(usize, usize, &Vec<CellType>, &Vec<CellType>), JobError> {
    let tables:Vec<(usize, &Vec<CellType>)> = input_type.iter().enumerate().flat_map(|(idx, t)| {
        match &t.cell_type {
            CellDataType::Output(sub_types) => Some((idx, sub_types)),
            _ => None,
        }
    }).collect();
    if tables.len() == 2 {
        Ok((tables[0].0, tables[1].0, tables[0].1, tables[1].1))
    } else {
        Err(argument_error(format!("Could not guess tables to join, expected two tables, found {}", tables.len()).as_str()))
    }
}

fn parse(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Config, JobError> {
    if (arguments.len() != 3) {
        return Err(argument_error("Expected exactly 3 aguments"));
    }
    return match (&arguments[0].cell, &arguments[1].cell, &arguments[2].cell) {
        (Cell::Field(l), Cell::Op(op), Cell::Field(r)) => {
            if op.as_str() != "==" {
                return Err(argument_error("Only == currently supported"));
            }
            match (l.matches('.').count(), r.matches('.').count()) {
                (0, 0) => {
                    let (left_table_idx, right_table_idx, left_types, right_types) = guess_tables(input_type)?;
                    Ok(Config {
                        left_table_idx,
                        right_table_idx,
                        left_column_idx: find_field(&l, left_types)?,
                        right_column_idx: find_field(&r, right_types)?,
                    })
                }
                (1, 1) => Err(argument_error("Not implemented yet!")),
                _ => Err(argument_error("Expected both fields on the form %table.column or %column")),
            }
        }
        _ => Err(argument_error("Expected arguments like %table1.col == %table2.col")),
    };
}

fn combine(mut l: Row, mut r: Row, cfg: &Config) -> Row {
    for (idx, c) in r.cells.drain(..).enumerate() {
        if idx != cfg.right_column_idx {
            l.cells.push(c);
        }
    }
    return Row {cells: l.cells}
}

fn do_join(cfg: &Config, l: &mut impl Readable, r: &mut impl Readable, output: &OutputStream) {
    let mut l_data: HashMap<Cell, Row> = HashMap::new();
    loop {
        match l.read() {
            Ok(row) => {
                l_data.insert(row.cells[cfg.left_column_idx].concrete(), row);
            }
            Err(_) => break,
        }
    }

    loop {
        match r.read() {
            Ok(r_row) => {
                l_data
                    .remove(&r_row.cells[cfg.right_column_idx])
                    .map(|l_row| {
                    output.send(combine( l_row, r_row, cfg));
                });
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
                        do_join(&cfg, &mut l.stream, &mut r.stream, &output);
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
        CellType::named("name", CellDataType::Text),
        CellType::named("age", CellDataType::Integer),
        CellType::named("home", CellDataType::Text),
    ];
    return Ok(Call {
        name: String::from("join"),
        input_type,
        arguments,
        output_type,
        exec: Exec::Run(run),
    });
}
