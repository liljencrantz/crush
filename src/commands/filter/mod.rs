mod parser;

use crate::{
    data::{
        Cell,
        CellType,
        Row,
        Argument
    },
    stream::{OutputStream, InputStream},
    commands::{Call, Exec},
    errors::{JobError, argument_error},
    glob::glob,
    commands::filter::parser::{Condition, Value, parse}
};
use std::iter::Iterator;

fn do_match(needle: &Cell, haystack: &Cell) -> bool {
    match (needle, haystack) {
        (Cell::Text(s), Cell::Glob(pattern)) => glob( pattern, s),
        (Cell::File(f), Cell::Glob(pattern)) => f.to_str().map(|s| glob( pattern, s)).unwrap(),
        (Cell::Text(s), Cell::Regex(_, pattern)) => pattern.is_match(s),
        (Cell::File(f), Cell::Regex(_, pattern)) => f.to_str().map(|s| pattern.is_match(s)).unwrap(),
        _ => panic!("Impossible")
    }
}

fn to_cell<'a>(value: &'a Value, row: &'a Row) -> &'a Cell {
    return match value {
        Value::Cell(c) => &c,
        Value::Field(idx) => &row.cells[*idx],
    };
}

fn evaluate(condition: &Condition, row: &Row) -> bool {
    return match condition {
        Condition::Equal(l, r) =>
            to_cell(&l, row) == to_cell(&r, row),
        Condition::GreaterThan(l, r) =>
            to_cell(&l, row) > to_cell(&r, row),
        Condition::GreaterThanOrEqual(l, r) =>
            to_cell(&l, row) >= to_cell(&r, row),
        Condition::LessThan(l, r) =>
            to_cell(&l, row) < to_cell(&r, row),
        Condition::LessThanOrEqual(l, r) =>
            to_cell(&l, row) <= to_cell(&r, row),
        Condition::NotEqual(l, r) =>
            to_cell(&l, row) != to_cell(&r, row),
        Condition::Match(l, r) =>
            do_match(to_cell(&l, row), to_cell(&r, row)),
        Condition::NotMatch(l, r) =>
            !do_match(to_cell(&l, row), to_cell(&r, row)),
    };
}

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream) -> Result<(), JobError> {
    let condition = parse(&input_type, &arguments)?;
    loop {
        match input.recv() {
            Ok(row) => {
                if evaluate(&condition, &row) {
                    output.send(row)?;
                }
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn filter(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    parse(&input_type, &arguments)?;
    return Ok(Call {
        name: String::from("filter"),
        output_type: input_type.clone(),
        input_type,
        arguments: arguments,
        exec: Exec::Run(run),
    });
}
