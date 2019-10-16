use crate::data::{CellType, Argument, CellDataType};
use crate::data::cell::Cell;
use crate::errors::{JobError, argument_error};
use crate::commands::command_util::find_field;

#[derive(Debug)]
pub enum Value {
    Cell(Cell),
    Field(usize),
}

#[derive(Debug)]
pub enum Condition {
    //    And(Box<Condition>, Box<Condition>),
//    Or(Box<Condition>, Box<Condition>),
    Equal(Value, Value),
    GreaterThan(Value, Value),
    GreaterThanOrEqual(Value, Value),
    LessThan(Value, Value),
    LessThanOrEqual(Value, Value),
    NotEqual(Value, Value),
    Match(Value, Value),
    NotMatch(Value, Value),
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
                _ => arg.cell.partial_clone().and_then({ |a| Ok(Value::Cell(a)) }),
            };
        }
        None => {
            return Err(argument_error("Expected one more value"));
        }
    }
}

fn to_cell_data_type(input_type: &Vec<CellType>, value: &Value) -> CellDataType {
    match value {
        Value::Cell(c) => c.cell_data_type(),
        Value::Field(idx) => input_type[*idx].cell_type.clone(),
    }
}

fn check_value(input_type: &Vec<CellType>, value: &Value, accepted_types: &Vec<CellDataType>) -> Option<JobError> {
    let t = to_cell_data_type(input_type, value);
    for a in accepted_types {
        if t == *a {
            return None;
        }
    }
    if accepted_types.len() == 1 {
        Some(argument_error(format!("Invalid cell type, saw {:?}, required {:?}", t, accepted_types[0]).as_str()))
    } else {
        Some(argument_error(format!("Invalid cell type, saw {:?}, required one of {:?}", t, accepted_types).as_str()))
    }
}

fn check_comparison(input_type: &Vec<CellType>, l: &Value, r: &Value) -> Option<JobError> {
    if let Some(err) = check_value(&input_type, r, &vec![to_cell_data_type(input_type, l)]) {
        return Some(err);
    }
    None
}

fn check_match(input_type: &Vec<CellType>, cond: Result<Condition, JobError>) -> Result<Condition, JobError> {
    match &cond {
        Ok(Condition::Match(l, r)) | Ok(Condition::NotMatch(l, r)) => {
            if let Some(err) = check_value(&input_type, r, &vec![CellDataType::Glob, CellDataType::Regex]) {
                return Err(err);
            }
            if let Some(err) = check_value(&input_type, l, &vec![CellDataType::Text, CellDataType::File]) {
                return Err(err);
            }
            cond
        }
        _ => cond,
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
                "==" => if let Some(e) = check_comparison(input_type, &val1, &val2) { Err(e) } else { Ok(Condition::Equal(val1, val2)) },
                ">" => if let Some(e) = check_comparison(input_type, &val1, &val2) { Err(e) } else { Ok(Condition::GreaterThan(val1, val2)) },
                ">=" => if let Some(e) = check_comparison(input_type, &val1, &val2) { Err(e) } else { Ok(Condition::GreaterThanOrEqual(val1, val2)) },
                "<" => if let Some(e) = check_comparison(input_type, &val1, &val2) { Err(e) } else { Ok(Condition::LessThan(val1, val2)) },
                "<=" => if let Some(e) = check_comparison(input_type, &val1, &val2) { Err(e) } else { Ok(Condition::LessThanOrEqual(val1, val2)) },
                "!=" => if let Some(e) = check_comparison(input_type, &val1, &val2) { Err(e) } else { Ok(Condition::NotEqual(val1, val2)) },
                "=~" => check_match(input_type, Ok(Condition::Match(val1, val2))),
                "!~" => check_match(input_type, Ok(Condition::NotMatch(val1, val2))),
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

pub fn parse(input_type: &Vec<CellType>,
             arguments: &Vec<Argument>) -> Result<Condition, JobError> {
    let lookup = find_checks(&input_type, &arguments)?;

    let numbered_arguments: Vec<(usize, &Argument)> = arguments.iter().enumerate().collect();
    return parse_condition(&input_type, &mut numbered_arguments.iter(), &lookup);
}
