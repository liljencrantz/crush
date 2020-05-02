use crate::lang::argument::Argument;
use crate::lang::errors::argument_error;
use crate::lang::errors::CrushError;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::{ArgumentVector, ExecutionContext};
use crate::lang::printer::Printer;
use crate::lang::r#struct::Struct;
use crate::lang::stream::OutputStream;
use crate::lang::stream::Readable;
use crate::lang::table::ColumnType;
use crate::lang::table::ColumnVec;
use crate::lang::table::Row;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::replace::Replace;
use std::collections::HashMap;

pub struct Config {
    left_table_idx: usize,
    right_table_idx: usize,
    left_column_idx: usize,
    right_column_idx: usize,
}

pub fn get_sub_type(cell_type: &ValueType) -> Result<&[ColumnType], CrushError> {
    match cell_type {
        ValueType::TableStream(sub_types) | ValueType::Table(sub_types) => Ok(sub_types),
        _ => argument_error("Expected a table column"),
    }
}

pub fn guess_tables(
    input_type: &[ColumnType],
) -> Result<(usize, usize, &[ColumnType], &[ColumnType]), CrushError> {
    let tables: Vec<(usize, &Vec<ColumnType>)> = input_type
        .iter()
        .enumerate()
        .flat_map(|(idx, t)| match &t.cell_type {
            ValueType::TableStream(sub_types) | ValueType::Table(sub_types) => {
                Some((idx, sub_types))
            }
            _ => None,
        })
        .collect();
    if tables.len() == 2 {
        Ok((tables[0].0, tables[1].0, tables[0].1, tables[1].1))
    } else {
        argument_error(
            format!(
                "Could not guess tables to join, expected two tables, found {}",
                tables.len()
            )
            .as_str(),
        )
    }
}

fn scan_table(
    table: &str,
    column: &str,
    input_type: &[ColumnType],
) -> Result<(usize, usize), CrushError> {
    let table_idx = input_type.find_str(&table.to_string())?;
    let column_idx =
        get_sub_type(&input_type[table_idx].cell_type)?.find_str(&column.to_string())?;
    Ok((table_idx, column_idx))
}

fn parse(input_type: &[ColumnType], arguments: Vec<Argument>) -> Result<Config, CrushError> {
    arguments.check_len(2)?;

    match (&arguments[0].value, &arguments[1].value) {
        (Value::Field(l), Value::Field(r)) => {
            let config = match (l.len(), r.len()) {
                (1, 1) => {
                    let (left_table_idx, right_table_idx, left_types, right_types) =
                        guess_tables(&input_type)?;

                    Config {
                        left_table_idx,
                        right_table_idx,
                        left_column_idx: left_types.find(&l)?,
                        right_column_idx: right_types.find(&r)?,
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
                _ => {
                    return argument_error(
                        "Expected both fields on the form %table.column or %column",
                    )
                }
            };

            let r_type = &get_sub_type(&input_type[config.right_table_idx].cell_type)?
                [config.right_column_idx]
                .cell_type;
            let l_type = &get_sub_type(&input_type[config.left_table_idx].cell_type)?
                [config.left_column_idx]
                .cell_type;
            if r_type != l_type {
                return argument_error("Cannot join two columns of different types");
            }
            if !r_type.is_hashable() {
                argument_error(
                    "Cannot join on this column type. (It is either mutable or not comparable)",
                )
            } else {
                Ok(config)
            }
        }
        _ => argument_error("Expected arguments like %table1.col == %table2.col"),
    }
}

fn combine(mut l: Row, r: Row, cfg: &Config) -> Row {
    for (idx, c) in r.into_vec().drain(..).enumerate() {
        if idx != cfg.right_column_idx {
            l.push(c);
        }
    }
    l
}

fn do_join(
    cfg: &Config,
    l: &mut dyn Readable,
    r: &mut dyn Readable,
    output: &OutputStream,
    printer: &Printer,
) -> CrushResult<()> {
    let mut l_data: HashMap<Value, Row> = HashMap::new();
    while let Ok(row) = l.read() {
        l_data.insert(row.cells()[cfg.left_column_idx].clone(), row);
    }

    while let Ok(r_row) = r.read() {
        l_data
            .remove(&r_row.cells()[cfg.right_column_idx])
            .map(|l_row| {
                printer.handle_error(output.send(combine(l_row, r_row, cfg)));
            });
    }
    Ok(())
}

pub fn run(
    config: Config,
    row: Struct,
    output: OutputStream,
    printer: &Printer,
) -> CrushResult<()> {
    let mut v = row.to_vec();
    match (
        v.replace(config.left_table_idx, Value::Integer(0))
            .readable(),
        v.replace(config.right_table_idx, Value::Integer(0))
            .readable(),
    ) {
        (Some(mut l), Some(mut r)) => do_join(&config, l.as_mut(), r.as_mut(), &output, printer),
        _ => panic!("Wrong row format"),
    }
}

fn get_output_type(input_type: &[ColumnType], cfg: &Config) -> Result<Vec<ColumnType>, CrushError> {
    let tables: Vec<Option<&Vec<ColumnType>>> = input_type
        .iter()
        .map(|t| match &t.cell_type {
            ValueType::TableStream(sub_types) | ValueType::Table(sub_types) => Some(sub_types),
            _ => None,
        })
        .collect();

    match (tables[cfg.left_table_idx], tables[cfg.right_table_idx]) {
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
    }
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Struct(s) => {
            let cfg = parse(&s.local_signature(), context.arguments)?;
            let output_type = get_output_type(&s.local_signature(), &cfg)?;
            let output = context.output.initialize(output_type)?;
            run(cfg, s, output, &context.printer)
        }
        _ => argument_error("Expected a struct"),
    }
}
