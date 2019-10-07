use crate::stream::{OutputStream, InputStream};
use crate::result::{Argument, CellType, Cell, Row};
use crate::commands::{InternalCall, Command, Call, InternalCommand};
use crate::errors::{JobError, parse_error, argument_error};
use crate::state::State;
use std::iter::Iterator;
use std::iter::Enumerate;
#[derive(Clone)]
pub struct Filter {}

fn find_field(needle: &String, haystack: &Vec<CellType>) -> Result<usize, JobError> {
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
}

fn parse_value(input_type: &Vec<CellType>,
               arguments: &mut std::slice::Iter<(usize, &Argument)>,
               field_lookup: &Vec<Option<usize>>) -> Result<Value, JobError> {
    match arguments.next() {
        Some((arg_idx, arg)) => {
            return match &arg.cell {
                Cell::Field(name) => Ok(Value::Field(field_lookup[*arg_idx].expect("Impossible"))),
                Cell::Op(_) => Err(argument_error("Expected value")),
                _ => return Ok(Value::Cell(arg.cell.clone())),
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
            return Ok(Condition::Equal(val1, val2));
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

fn to_cell(value: &Value, row: &Row) -> Cell {
    return match value {
        Value::Cell(c) => c.clone(),
        Value::Field(idx) => row.cells[*idx].clone(),
    }
}

fn evaluate(condition: &Condition, row: &Row) -> bool {
    match condition {
        Condition::Equal(l, r) => {
            return to_cell(&l, row) == to_cell(&r, row);
        }
    }
}

impl InternalCommand for Filter {
    fn run(
        &mut self,
        _state: &State,
        input_type: &Vec<CellType>,
        arguments: &Vec<Argument>,
        input: &mut dyn InputStream,
        output: &mut dyn OutputStream) -> Result<(), JobError> {
        let lookup = find_checks(input_type, arguments)?;

        let numbered_arguments: Vec<(usize, &Argument)> = arguments.iter().enumerate().collect();
        let condition = parse_condition(input_type, &mut numbered_arguments.iter(), &lookup)?;
//        println!("WEE {:?}", eval);
        loop {
            match input.next() {
                Some(row) => {
                    let mut ok = evaluate(&condition, &row);
                    if ok {
                        output.add(row);
                    }
                }
                None => {
                    break;
                }
            }
        }
        return Ok(());
    }
}

impl Command for Filter {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Box<dyn Call>, JobError> {
        return Ok(Box::new(InternalCall {
            name: String::from("filter"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type: input_type.clone(),
            command: Box::new(self.clone()),
        }));
    }
}
