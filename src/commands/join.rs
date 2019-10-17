use std::collections::HashMap;
use crate::{
    stream::Readable,
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
    replace::Replace,
    errors::argument_error,
    commands::command_util::find_field
};

struct Config {
    left_table_idx: usize,
    right_table_idx: usize,
    left_column_idx: usize,
    right_column_idx: usize,
}

pub fn get_sub_type(cell_type: &CellDataType) -> Result<&Vec<CellType>, JobError>{
    match cell_type {
        CellDataType::Output(sub_types) | CellDataType::Rows(sub_types)
        | CellDataType::Row(sub_types) => Ok(sub_types),
        _ => Err(argument_error("Expected a table column")),
    }
}

pub fn guess_tables(input_type: &Vec<CellType>) -> Result<(usize, usize, &Vec<CellType>, &Vec<CellType>), JobError> {
    let tables: Vec<(usize, &Vec<CellType>)> = input_type.iter().enumerate().flat_map(|(idx, t)| {
        match &t.cell_type {
            CellDataType::Output(sub_types) | CellDataType::Rows(sub_types)
            | CellDataType::Row(sub_types) => Some((idx, sub_types)),
            _ => None,
        }
    }).collect();
    if tables.len() == 2 {
        Ok((tables[0].0, tables[1].0, tables[0].1, tables[1].1))
    } else {
        Err(argument_error(format!("Could not guess tables to join, expected two tables, found {}", tables.len()).as_str()))
    }
}

fn scan_table(table: &str, column: &str, input_type: &Vec<CellType>) -> Result<(usize, usize), JobError> {
    let table_idx = find_field(&table.to_string(), input_type)?;
    let column_idx = find_field(&column.to_string(), get_sub_type(&input_type[table_idx].cell_type)?)?;
    Ok((table_idx, column_idx))
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
                (1, 1) => {
                    let left_split: Vec<&str> = l.split('.').collect();
                    let (left_table_idx, left_column_idx ) =
                        scan_table(left_split[0], left_split[1], input_type)?;

                    let right_split: Vec<&str> = r.split('.').collect();
                    let (right_table_idx, right_column_idx ) =
                        scan_table(right_split[0], right_split[1], input_type)?;

                    if left_table_idx == right_table_idx {
                        return Err(argument_error("Left and right table can't be the same"));
                    }

                    Ok(Config {
                        left_table_idx,
                        right_table_idx,
                        left_column_idx,
                        right_column_idx,
                    })
                },
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
    return Row { cells: l.cells };
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
                        output.send(combine(l_row, r_row, cfg));
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

fn get_output_type(input_type: &Vec<CellType>, cfg: &Config) -> Result<Vec<CellType>, JobError> {
    let tables: Vec<Option<&Vec<CellType>>> = input_type.iter().map(|t| {
        match &t.cell_type {
            CellDataType::Output(sub_types) | CellDataType::Rows(sub_types)
            | CellDataType::Row(sub_types) => Some(sub_types),
            _ => None,
        }
    }).collect();

    return match (tables[cfg.left_table_idx], tables[cfg.right_table_idx]) {
        (Some(v1), Some(v2)) => {
            let mut res = v1.clone();
            for (idx, c) in v2.iter().enumerate() {
                if idx != cfg.right_column_idx {
                    res.push(c.clone());
                }
            }
            Ok(res)
        }
        _ => Err(argument_error("Impossible error?"))
    };
}

pub fn join(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    let cfg = parse(&input_type, &arguments)?;
    return Ok(Call {
        name: String::from("join"),
        output_type: get_output_type(&input_type, &cfg)?,
        input_type,
        arguments,
        exec: Exec::Run(run),
    });
}
