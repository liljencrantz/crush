use crate::{
    data::{
        Argument,
        Value,
        ValueType,
    },
    errors::{argument_error, JobError},
};
use crate::data::ColumnType;
use crate::commands::command_util::find_field;
use crate::errors::error;

pub enum WhereValue {
    Value(Value),
    Field(usize),
}

pub enum Condition {
    And(Box<Condition>, Box<Condition>),
    Or(Box<Condition>, Box<Condition>),
    Equal(WhereValue, WhereValue),
    GreaterThan(WhereValue, WhereValue),
    GreaterThanOrEqual(WhereValue, WhereValue),
    LessThan(WhereValue, WhereValue),
    LessThanOrEqual(WhereValue, WhereValue),
    NotEqual(WhereValue, WhereValue),
    Match(WhereValue, WhereValue),
    NotMatch(WhereValue, WhereValue),
}

fn parse_value(input_type: &Vec<ColumnType>,
               arg: Argument) -> Result<WhereValue, JobError> {
    match arg.value {
        Value::Field(s) => Ok(WhereValue::Field(find_field(&s, input_type)?)),
        Value::Op(_) => Err(argument_error("Expected value")),
        Value::Output(_) => Err(argument_error("Invalid argument type Stream")),
        _ => Ok(WhereValue::Value(arg.value)),
    }
}

fn to_cell_data_type(input_type: &Vec<ColumnType>, value: &WhereValue) -> ValueType {
    match value {
        WhereValue::Value(c) => c.value_type(),
        WhereValue::Field(idx) => input_type[*idx].cell_type.clone(),
    }
}

fn check_value(input_type: &Vec<ColumnType>, value: &WhereValue, accepted_types: &Vec<ValueType>) -> Option<JobError> {
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

fn check_comparable(input_type: &Vec<ColumnType>, value: &WhereValue) -> bool {
    let t = to_cell_data_type(input_type, value);
    return t.is_comparable();
}

fn check_comparison(input_type: &Vec<ColumnType>, l: &WhereValue, r: &WhereValue) -> Option<JobError> {
    if !check_comparable(input_type, l) {
        return Some(error("Type can't be compared"));
    }
    if let Some(err) = check_value(&input_type, r, &vec![to_cell_data_type(input_type, l)]) {
        return Some(err);
    }
    None
}

fn check_match(input_type: &Vec<ColumnType>, cond: Result<Condition, JobError>) -> Result<Condition, JobError> {
    match &cond {
        Ok(Condition::Match(l, r)) | Ok(Condition::NotMatch(l, r)) => {
            if let Some(err) = check_value(&input_type, r, &vec![ValueType::Glob, ValueType::Regex]) {
                return Err(err);
            }
            if let Some(err) = check_value(&input_type, l, &vec![ValueType::Text, ValueType::File]) {
                return Err(err);
            }
        }
        _ => {}
    }
    cond
}

fn parse_value_condition(input_type: &Vec<ColumnType>,
                         arguments: &mut Vec<Argument>) -> Result<Condition, JobError> {
    if arguments.len() < 3 {
        return Err(error("Wrong number of arguments"));
    }
    let val1 = parse_value(input_type, arguments.remove(0))?;
    match arguments.remove(0).value {
        Value::Op(op) => {
            let val2 = parse_value(input_type, arguments.remove(0))?;
            return match op.as_ref() {
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

fn parse_bool_condition(input_type: &Vec<ColumnType>,
                        arguments: &mut Vec<Argument>) -> Result<Condition, JobError> {
    let cond1 = parse_value_condition(input_type, arguments)?;
    if arguments.len() < 2 {
       return Ok(cond1);
    }
    match arguments.remove(0).value {
        Value::Text(op) => {
            let cond2 = parse_value_condition(input_type, arguments)?;
            return match op.as_ref() {
                "and" => Ok(Condition::And(Box::from(cond1), Box::from(cond2))),
                "or" => Ok(Condition::Or(Box::from(cond1), Box::from(cond2))),
                other => Err(argument_error(format!("Unknown comparison operation {}", other).as_str())),
            };
        }
        _ => return Err(argument_error("Expected comparison"))
    }
}

pub fn parse(input_type: &Vec<ColumnType>,
             arguments: &mut Vec<Argument>) -> Result<Condition, JobError> {
    let numbered_arguments: Vec<(usize, &Argument)> = arguments.iter().enumerate().collect();
    return parse_bool_condition(&input_type, arguments);
}
