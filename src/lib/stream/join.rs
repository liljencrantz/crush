use crate::lang::command::ExecutionContext;
use crate::lang::errors::CrushResult;
use std::collections::HashMap;
use crate::{
    lang::stream::Readable,
    lang::errors::CrushError,
    lang::{
        argument::Argument,
        table::Row,
        table::ColumnType,
        value::ValueType,
        value::Value,
    },
    lang::stream::OutputStream,
    util::replace::Replace,
    lang::errors::argument_error,
    lib::command_util::find_field_from_str,
};
use crate::lib::command_util::find_field;
use crate::lang::r#struct::Struct;

pub struct Config {
    left_table_idx: usize,
    right_table_idx: usize,
    left_column_idx: usize,
    right_column_idx: usize,
}

pub fn get_sub_type(cell_type: &ValueType) -> Result<&Vec<ColumnType>, CrushError> {
    match cell_type {
        ValueType::TableStream(sub_types) | ValueType::Table(sub_types) => Ok(sub_types),
        _ => argument_error("Expected a table column"),
    }
}

pub fn guess_tables(input_type: &Vec<ColumnType>) -> Result<(usize, usize, &Vec<ColumnType>, &Vec<ColumnType>), CrushError> {
    let tables: Vec<(usize, &Vec<ColumnType>)> = input_type.iter().enumerate().flat_map(|(idx, t)| {
        match &t.cell_type {
            ValueType::TableStream(sub_types) | ValueType::Table(sub_types) => Some((idx, sub_types)),
            _ => None,
        }
    }).collect();
    if tables.len() == 2 {
        Ok((tables[0].0, tables[1].0, tables[0].1, tables[1].1))
    } else {
        argument_error(format!("Could not guess tables to join, expected two tables, found {}", tables.len()).as_str())
    }
}

fn scan_table(table: &str, column: &str, input_type: &Vec<ColumnType>) -> Result<(usize, usize), CrushError> {
    let table_idx = find_field_from_str(&table.to_string(), input_type)?;
    let column_idx = find_field_from_str(&column.to_string(), get_sub_type(&input_type[table_idx].cell_type)?)?;
    Ok((table_idx, column_idx))
}

fn parse(input_type: &Vec<ColumnType>, arguments: Vec<Argument>) -> Result<Config, CrushError> {
    if arguments.len() != 2 {
        return argument_error("Expected exactly 2 aguments");
    }
    return match (&arguments[0].value, &arguments[1].value) {
        (Value::Field(l), Value::Field(r)) => {

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
                    let (left_table_idx, left_column_idx) =
                        scan_table(l[0].as_ref(), l[1].as_ref(), &input_type)?;

                    let (right_table_idx, right_column_idx) =
                        scan_table(r[0].as_ref(), r[1].as_ref(), &input_type)?;

                    if left_table_idx == right_table_idx {
                        return argument_error("Left and right table can't be the same");
                    }

                    Config {
                        left_table_idx,
                        right_table_idx,
                        left_column_idx,
                        right_column_idx,
                    }
                }
                _ => return argument_error("Expected both fields on the form %table.column or %column"),
            };

            let r_type = &get_sub_type(&input_type[config.right_table_idx].cell_type)?[config.right_column_idx].cell_type;
            let l_type = &get_sub_type(&input_type[config.left_table_idx].cell_type)?[config.left_column_idx].cell_type;
            if r_type != l_type {
                return argument_error("Cannot join two columns of different types");
            }
            if !r_type.is_hashable() {
                return argument_error("Cannot join on this column type. (It is either mutable or not comparable)");
            }
            Ok(config)
        }
        _ => argument_error("Expected arguments like %table1.col == %table2.col"),
    };
}

fn combine(mut l: Row, mut r: Row, cfg: &Config) -> Row {
    for (idx, c) in r.into_vec().drain(..).enumerate() {
        if idx != cfg.right_column_idx {
            l.push(c);
        }
    }
    return l;
}

fn do_join(cfg: &Config, l: &mut dyn Readable, r: &mut dyn Readable, output: &OutputStream) -> CrushResult<()> {
    let mut l_data: HashMap<Value, Row> = HashMap::new();
    loop {
        match l.read() {
            Ok(row) => {
                l_data.insert(row.cells()[cfg.left_column_idx].clone(), row);
            }
            Err(_) => break,
        }
    }

    loop {
        match r.read() {
            Ok(r_row) => {
                l_data
                    .remove(&r_row.cells()[cfg.right_column_idx])
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
    mut row: Struct,
    output: OutputStream,
) -> CrushResult<()> {
    let mut v = row.into_vec();
    match (v.replace(config.left_table_idx, Value::Integer(0)).readable(), v.replace(config.right_table_idx, Value::Integer(0)).readable()) {
        (Some(mut l), Some(mut r)) => {
            do_join(&config, l.as_mut(), r.as_mut(), &output)?;
        }
        _ => panic!("Wrong row format"),
    }
    Ok(())
}

fn get_output_type(input_type: &Vec<ColumnType>, cfg: &Config) -> Result<Vec<ColumnType>, CrushError> {
    let tables: Vec<Option<&Vec<ColumnType>>> = input_type.iter().map(|t| {
        match &t.cell_type {
            ValueType::TableStream(sub_types) | ValueType::Table(sub_types) => Some(sub_types),
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
        _ => argument_error("Impossible error?"),
    };
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Struct(s) => {
            let cfg = parse(s.types(), context.arguments)?;
            let output_type = get_output_type(s.types(), &cfg)?;
            let output = context.output.initialize(output_type)?;
            run(cfg, s, output)
        }
        _ => argument_error("Expected a struct"),
    }
}
