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
    commands::filter::parser::{Condition, Value, parse}
};
use std::iter::Iterator;
use crate::printer::Printer;
use crate::errors::error;
use std::cmp::Ordering;
use crate::data::{ConcreteCell, ConcreteRow};
use crate::state::State;

fn do_match(needle: &ConcreteCell, haystack: &ConcreteCell) -> Result<bool, JobError> {
    match (needle, haystack) {
        (ConcreteCell::Text(s), ConcreteCell::Glob(pattern)) => Ok(pattern.matches( s)),
        (ConcreteCell::File(f), ConcreteCell::Glob(pattern)) => f.to_str().map(|s| Ok(pattern.matches( s))).unwrap_or(Err(error("Invalid filename"))),
        (ConcreteCell::Text(s), ConcreteCell::Regex(_, pattern)) => Ok(pattern.is_match(s)),
        (ConcreteCell::File(f), ConcreteCell::Regex(_, pattern)) => match f.to_str().map(|s| pattern.is_match(s)) {
            Some(v) => Ok(v),
            None => Err(error("Invalid filename")),
        },
        _ => Err(error("Invalid match"))
    }
}

fn to_cell<'a>(value: &'a Value, row: &'a ConcreteRow) -> &'a ConcreteCell {
    return match value {
        Value::Cell(c) => &c,
        Value::Field(idx) => &row.cells[*idx],
    };
}

fn evaluate(condition: &Condition, row: &ConcreteRow) -> Result<bool, JobError> {
    return match condition {
        Condition::Equal(l, r) =>
            Ok(to_cell(&l, row) == to_cell(&r, row)),
        Condition::GreaterThan(l, r) =>
            match to_cell(&l, row).partial_cmp(to_cell(&r, row)) {
                Some(ord) => Ok(ord == Ordering::Greater),
                None => Err(error("Cell types can't be compared")),
            },
        Condition::GreaterThanOrEqual(l, r) =>
            match to_cell(&l, row).partial_cmp(to_cell(&r, row)) {
                Some(ord) => Ok(ord != Ordering::Less),
                None => Err(error("Cell types can't be compared")),
            },
        Condition::LessThan(l, r) =>
            match to_cell(&l, row).partial_cmp(to_cell(&r, row)) {
                Some(ord) => Ok(ord == Ordering::Less),
                None => Err(error("Cell types can't be compared")),
            },
        Condition::LessThanOrEqual(l, r) =>
            match to_cell(&l, row).partial_cmp(to_cell(&r, row)) {
                Some(ord) => Ok(ord != Ordering::Greater),
                None => Err(error("Cell types can't be compared")),
            },
        Condition::NotEqual(l, r) =>
            Ok(to_cell(&l, row) != to_cell(&r, row)),
        Condition::Match(l, r) =>
            do_match(to_cell(&l, row), to_cell(&r, row)),
        Condition::NotMatch(l, r) =>
            do_match(to_cell(&l, row), to_cell(&r, row)).map(|r| !r),
    };
}

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
    state: State,
    printer: Printer,
) -> Result<(), JobError> {
    let condition = parse(&input_type, &arguments)?;
    loop {
        match input.recv() {
            Ok(row) => {
                match evaluate(&condition, &row.concrete_copy()) {
                    Ok(val) => if output.send(row).is_err() { break },
                    Err(e) => printer.job_error(e),
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
        exec: Exec::Command(run),
    });
}
