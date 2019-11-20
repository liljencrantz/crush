use crate::commands::CompileContext;
use crate::errors::JobResult;
use std::collections::HashMap;
use crate::{
    stream::Readable,
    errors::JobError,
    data::{
        Argument,
        Row,
        ColumnType,
        CellType,
        Cell,
    },
    stream::{OutputStream, InputStream},
    replace::Replace,
    errors::argument_error,
    commands::command_util::find_field_from_str
};
use crate::commands::command_util::find_field;
use crate::stream::RowsReader;

pub struct Config {
    left_table_idx: usize,
    right_table_idx: usize,
    left_column_idx: usize,
    right_column_idx: usize,
}

pub fn get_sub_type(cell_type: &CellType) -> Result<&Vec<ColumnType>, JobError>{
    match cell_type {
        CellType::Output(sub_types) | CellType::Rows(sub_types) => Ok(sub_types),
        _ => Err(argument_error("Expected a table column")),
    }
}

pub fn guess_tables(input_type: &Vec<ColumnType>) -> Result<(usize, usize, &Vec<ColumnType>, &Vec<ColumnType>), JobError> {
    let tables: Vec<(usize, &Vec<ColumnType>)> = input_type.iter().enumerate().flat_map(|(idx, t)| {
        match &t.cell_type {
            CellType::Output(sub_types) | CellType::Rows(sub_types) => Some((idx, sub_types)),
            _ => None,
        }
    }).collect();
    if tables.len() == 2 {
        Ok((tables[0].0, tables[1].0, tables[0].1, tables[1].1))
    } else {
        Err(argument_error(format!("Could not guess tables to join, expected two tables, found {}", tables.len()).as_str()))
    }
}

fn scan_table(table: &str, column: &str, input_type: &Vec<ColumnType>) -> Result<(usize, usize), JobError> {
    let table_idx = find_field_from_str(&table.to_string(), input_type)?;
    let column_idx = find_field_from_str(&column.to_string(), get_sub_type(&input_type[table_idx].cell_type)?)?;
    Ok((table_idx, column_idx))
}

fn parse(input_type: Vec<ColumnType>, arguments: Vec<Argument>) -> Result<Config, JobError> {
    if arguments.len() != 3 {
        return Err(argument_error("Expected exactly 3 aguments"));
    }
    return match (&arguments[0].cell, &arguments[1].cell, &arguments[2].cell) {
        (Cell::Field(l), Cell::Op(op), Cell::Field(r)) => {
            if op.as_ref() != "==" {
                return Err(argument_error("Only == currently supported"));
            }

            let config = match (l.len(), r.len()) {
                (1, 1) => {
                    let (left_table_idx, right_table_idx, left_types, right_types) = guess_tables(&input_type)?;

                    Config {
                        left_table_idx,
                        right_table_idx,
                        left_column_idx: find_field(&l, left_types)?,
                        right_column_idx: find_field(&r, right_types)?,
                    }
                }
                (2, 2) => {
                    let (left_table_idx, left_column_idx ) =
                        scan_table(l[0].as_ref(), l[1].as_ref(), &input_type)?;

                    let (right_table_idx, right_column_idx ) =
                        scan_table(r[0].as_ref(), r[1].as_ref(), &input_type)?;

                    if left_table_idx == right_table_idx {
                        return Err(argument_error("Left and right table can't be the same"));
                    }

                    Config {
                        left_table_idx,
                        right_table_idx,
                        left_column_idx,
                        right_column_idx,
                    }
                },
                _ => return Err(argument_error("Expected both fields on the form %table.column or %column")),
            };

            let r_type = &get_sub_type(&input_type[config.right_table_idx].cell_type)?[config.right_column_idx].cell_type;
            let l_type = &get_sub_type(&input_type[config.left_table_idx].cell_type)?[config.left_column_idx].cell_type;
            if r_type != l_type {
                return Err(argument_error("Cannot join two columns of different types"));
            }
            if !r_type.is_hashable() {
                return Err(argument_error("Cannot join on this column type. (It is either mutable or not comparable)"));
            }
            Ok(config)
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

fn do_join(cfg: &Config, l: &mut impl Readable, r: &mut impl Readable, output: &OutputStream) -> JobResult<()>{
    let mut l_data: HashMap<Cell, Row> = HashMap::new();
    loop {
        match l.read() {
            Ok(row) => {
                l_data.insert(row.cells[cfg.left_column_idx].partial_clone()?, row);
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
    Ok(())
}

pub fn run(
    config: Config,
    input: InputStream,
    output: OutputStream,
) -> JobResult<()> {
    loop {
        match input.recv() {
            Ok(mut row) => {
                match (row.cells.replace(config.left_table_idx, Cell::Integer(0)), row.cells.replace(config.right_table_idx, Cell::Integer(0))) {
                    (Cell::Output(mut l), Cell::Output(mut r)) => {
                        do_join(&config, &mut l.stream, &mut r.stream, &output)?;
                    }
                    (Cell::Rows(mut l), Cell::Rows(mut r)) => {
                        do_join(&config, &mut l.reader(), &mut r.reader(), &output)?;
                    }
                    (Cell::Output(mut l), Cell::Rows(mut r)) => {
                        do_join(&config, &mut l.stream, &mut r.reader(), &output)?;
                    }
                    (Cell::Rows(mut l), Cell::Output(mut r)) => {
                        do_join(&config, &mut l.reader(), &mut r.stream, &output)?;
                    }
                    _ => panic!("Wrong row format"),
                }
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

fn get_output_type(input_type: &Vec<ColumnType>, cfg: &Config) -> Result<Vec<ColumnType>, JobError> {
    let tables: Vec<Option<&Vec<ColumnType>>> = input_type.iter().map(|t| {
        match &t.cell_type {
            CellType::Output(sub_types) | CellType::Rows(sub_types) => Some(sub_types),
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

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize_stream()?;
    let cfg = parse(input.get_type().clone(), context.arguments)?;
    let output_type = get_output_type(input.get_type(), &cfg)?;
    let output = context.output.initialize(output_type)?;
    run(cfg, input, output)
}
