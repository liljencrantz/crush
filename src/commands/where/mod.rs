use std::cmp::Ordering;

use crate::{
    commands::r#where::parser::{Condition, parse, Value},
    data::{
        Cell,
        Row,
    },
    stream::{InputStream, OutputStream}
};
use crate::commands::CompileContext;
use crate::errors::{error, JobResult};
use crate::printer::Printer;

mod parser;

pub struct Config {
    condition: Condition,
    input: InputStream,
    output: OutputStream,
}


fn do_match(needle: &Cell, haystack: &Cell) -> JobResult<bool> {
    match (needle, haystack) {
        (Cell::Text(s), Cell::Glob(pattern)) => Ok(pattern.matches( s)),
        (Cell::File(f), Cell::Glob(pattern)) => f.to_str().map(|s| Ok(pattern.matches( s))).unwrap_or(Err(error("Invalid filename"))),
        (Cell::Text(s), Cell::Regex(_, pattern)) => Ok(pattern.is_match(s)),
        (Cell::File(f), Cell::Regex(_, pattern)) => match f.to_str().map(|s| pattern.is_match(s)) {
            Some(v) => Ok(v),
            None => Err(error("Invalid filename")),
        },
        _ => Err(error("Invalid match"))
    }
}

fn to_cell<'a>(value: &'a Value, row: &'a Row) -> &'a Cell {
    return match value {
        Value::Cell(c) => &c,
        Value::Field(idx) => &row.cells[*idx],
    };
}

fn evaluate(condition: &Condition, row: &Row) -> JobResult<bool> {
    Ok(match condition {
        Condition::Equal(l, r) =>
            to_cell(&l, row) == to_cell(&r, row),
        Condition::GreaterThan(l, r) =>
            match to_cell(&l, row).partial_cmp(to_cell(&r, row)) {
                Some(ord) => Ok(ord == Ordering::Greater),
                None => Err(error("Cell types can't be compared")),
            }?,
        Condition::GreaterThanOrEqual(l, r) =>
            match to_cell(&l, row).partial_cmp(to_cell(&r, row)) {
                Some(ord) => Ok(ord != Ordering::Less),
                None => Err(error("Cell types can't be compared")),
            }?,
        Condition::LessThan(l, r) =>
            match to_cell(&l, row).partial_cmp(to_cell(&r, row)) {
                Some(ord) => Ok(ord == Ordering::Less),
                None => Err(error("Cell types can't be compared")),
            }?,
        Condition::LessThanOrEqual(l, r) =>
            match to_cell(&l, row).partial_cmp(to_cell(&r, row)) {
                Some(ord) => Ok(ord != Ordering::Greater),
                None => Err(error("Cell types can't be compared")),
            }?,
        Condition::NotEqual(l, r) =>
            to_cell(&l, row) != to_cell(&r, row),
        Condition::Match(l, r) =>
            do_match(to_cell(&l, row), to_cell(&r, row))?,
        Condition::NotMatch(l, r) =>
            do_match(to_cell(&l, row), to_cell(&r, row)).map(|r| !r)?,
        Condition::And(c1, c2) => evaluate(c1, row)? && evaluate(c2, row)?,
        Condition::Or(c1, c2) => evaluate(c1, row)? || evaluate(c2, row)?,
    })
}

pub fn run(config: Config, printer: Printer) -> JobResult<()> {
    loop {
        match config.input.recv() {
            Ok(row) => {
                match evaluate(&config.condition, &row) {
                    Ok(val) => if val { if config.output.send(row).is_err() { break }},
                    Err(e) => printer.job_error(e),
                }
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn compile_and_run(mut context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize()?;
    let output = context.output.initialize(input.get_type().clone())?;
    let config = Config {
        condition: parse(input.get_type(), context.arguments.as_mut())?,
        input: input,
        output: output,
    };
    run(config, context.printer)
}
