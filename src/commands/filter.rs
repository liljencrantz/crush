use crate::stream::{OutputStream, InputStream};
use crate::cell::{Argument, CellType, Cell, Row};
use crate::commands::{Call, Exec};
use crate::errors::{JobError, argument_error};
use crate::state::State;
use std::iter::Iterator;

pub fn find_field(needle: &String, haystack: &Vec<CellType>) -> Result<usize, JobError> {
    for (idx, field) in haystack.iter().enumerate() {
        if field.name.eq(needle) {
            return Ok(idx);
        }
    }
    return Err(JobError { message: String::from(format!("Unknown column \"{}\"", needle)) });
}

#[derive(Debug)]
enum Value {
    Cell(Cell),
    Field(usize),
}

#[derive(Debug)]
enum Condition {
    //    And(Box<Condition>, Box<Condition>),
//    Or(Box<Condition>, Box<Condition>),
    Equal(Value, Value),
    GreaterThan(Value, Value),
    GreaterThanOrEqual(Value, Value),
    LessThan(Value, Value),
    LessThanOrEqual(Value, Value),
    NotEqual(Value, Value),
}

fn parse_value(input_type: &Vec<CellType>,
               arguments: &mut std::slice::Iter<(usize, &Argument)>,
               field_lookup: &Vec<Option<usize>>) -> Result<Value, JobError> {
    match arguments.next() {
        Some((arg_idx, arg)) => {
            return match &arg.cell {
                Cell::Field(_) => Ok(Value::Field(field_lookup[*arg_idx].expect("Impossible"))),
                Cell::Op(_) => Err(argument_error("Expected value")),
                Cell::Output(_) => Err(argument_error("Invalid argument type Stream")),
                _ => arg.cell.partial_clone().and_then({|a| Ok(Value::Cell(a)) }),
            };
        }
        None => {
            return Err(argument_error("Expected one more value"));
        }
    }
}

fn parse_condition(input_type: &Vec<CellType>,
                   arguments: &mut std::slice::Iter<(usize, &Argument)>,
                   field_lookup: &Vec<Option<usize>>) -> Result<Condition, JobError> {
    let val1 = parse_value(input_type, arguments, field_lookup)?;
    match &arguments.next().ok_or(argument_error("Expected condition"))?.1.cell {
        Cell::Op(op) => {
            let val2 = parse_value(input_type, arguments, field_lookup)?;
            return match op.as_str() {
                "==" => Ok(Condition::Equal(val1, val2)),
                ">" => Ok(Condition::GreaterThan(val1, val2)),
                ">=" => Ok(Condition::GreaterThanOrEqual(val1, val2)),
                "<" => Ok(Condition::LessThan(val1, val2)),
                "<=" => Ok(Condition::LessThanOrEqual(val1, val2)),
                "!=" => Ok(Condition::NotEqual(val1, val2)),
                other => Err(argument_error(format!("Unknown comparison operation {}", other).as_str())),
            };
        }
        _ => return Err(argument_error("Expected comparison"))
    }
}

fn find_checks(input_type: &Vec<CellType>,
               arguments: &Vec<Argument>) -> Result<Vec<Option<usize>>, JobError> {
    let mut res: Vec<Option<usize>> = Vec::new();
    for arg in arguments {
        match &arg.cell {
            Cell::Field(val) => {
                res.push(Some(find_field(&val, input_type)?));
            }
            _ => {
                res.push(None);
            }
        }
    }
    return Ok(res);
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
    };
}

    fn run(
        input_type: Vec<CellType>,
        arguments: Vec<Argument>,
        input: InputStream,
        output: OutputStream) -> Result<(), JobError> {
        let lookup = find_checks(&input_type, &arguments)?;

        let numbered_arguments: Vec<(usize, &Argument)> = arguments.iter().enumerate().collect();
        let condition = parse_condition(&input_type, &mut numbered_arguments.iter(), &lookup)?;
        loop {
            match input.recv() {
                Ok(row) => {
                    if evaluate(&condition, &row) {
                        output.send(row);
                    }
                }
                Err(_) => {
                    break;
                }
            }
        }
        return Ok(());
    }

    pub fn filter(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
        return Ok(Call {
            name: String::from("filter"),
            input_type: input_type.clone(),
            arguments: arguments,
            output_type: input_type,
            exec: Exec::Run(run),
        });
    }
