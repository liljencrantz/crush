use std::cmp::Ordering;

use crate::{
    commands::r#where::parser::{Condition, parse, WhereValue},
    data::{
        Value,
        Row,
    },
    stream::{OutputStream}
};
use crate::commands::CompileContext;
use crate::errors::{error, CrushResult};
use crate::printer::Printer;
use crate::stream::Readable;
use crate::data::RowsReader;

mod parser;

pub struct Config<T: Readable> {
    condition: Condition,
    input: T,
    output: OutputStream,
}


fn do_match(needle: &Value, haystack: &Value) -> CrushResult<bool> {
    match (needle, haystack) {
        (Value::Text(s), Value::Glob(pattern)) => Ok(pattern.matches( s)),
        (Value::File(f), Value::Glob(pattern)) => f.to_str().map(|s| Ok(pattern.matches( s))).unwrap_or(Err(error("Invalid filename"))),
        (Value::Text(s), Value::Regex(_, pattern)) => Ok(pattern.is_match(s)),
        (Value::File(f), Value::Regex(_, pattern)) => match f.to_str().map(|s| pattern.is_match(s)) {
            Some(v) => Ok(v),
            None => Err(error("Invalid filename")),
        },
        _ => Err(error("Invalid match"))
    }
}

fn to_cell<'a>(value: &'a WhereValue, row: &'a Row) -> &'a Value {
    return match value {
        WhereValue::Value(c) => &c,
        WhereValue::Field(idx) => &row.cells()[*idx],
    };
}

fn evaluate(condition: &Condition, row: &Row) -> CrushResult<bool> {
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

pub fn run<T: Readable>(mut config: Config<T>, printer: Printer) -> CrushResult<()> {
    loop {
        match config.input.read() {
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

pub fn perform(mut context: CompileContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Stream(input) => {
            let output = context.output.initialize(input.stream.get_type().clone())?;
            let config = Config {
                condition: parse(input.stream.get_type(), context.arguments.as_mut())?,
                input: input.stream,
                output: output,
            };
            run(config, context.printer)
        }
        Value::Rows(r) => {
            let input = RowsReader::new(r);
            let output = context.output.initialize(input.get_type().clone())?;
            let config = Config {
                condition: parse(input.get_type(), context.arguments.as_mut())?,
                input: input,
                output: output,
            };
            run(config, context.printer)
        }
        _ => Err(error("Expected a stream")),
    }


}
