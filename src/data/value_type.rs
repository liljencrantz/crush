use crate::errors::{error, mandate, JobResult};
use crate::data::{Value, ColumnType, value_type_parser};
use crate::glob::Glob;
use regex::Regex;
use std::error::Error;
use crate::parser::parse_name;

#[derive(Clone)]
#[derive(PartialEq)]
#[derive(Eq)]
#[derive(PartialOrd)]
#[derive(Ord)]
#[derive(Debug)]
pub enum ValueType {
    Text,
    Integer,
    Time,
    Duration,
    Field,
    Glob,
    Regex,
    Op,
    Command,
    Closure,
    File,
    Output(Vec<ColumnType>),
    Rows(Vec<ColumnType>),
    Row(Vec<ColumnType>),
    List(Box<ValueType>),
    Dict(Box<ValueType>, Box<ValueType>),
    Env,
    Bool,
}



impl ValueType {
    pub fn from(s: &str) -> JobResult<ValueType> {
        value_type_parser::parse(s)
    }

    fn materialize_vec(input: &Vec<ValueType>) -> Vec<ValueType> {
        input
            .iter()
            .map(|cell| cell.materialize())
            .collect()
    }

    pub fn materialize(&self) -> ValueType {
        match self {
            ValueType::Text|
            ValueType::Integer|
            ValueType::Time |
            ValueType::Duration |
            ValueType::Field |
            ValueType::Glob |
            ValueType::Regex |
            ValueType::Op |
            ValueType::Command |
            ValueType::Closure |
            ValueType::File |
            ValueType::Env |
            ValueType::Bool => self.clone(),
            ValueType::Output(o) => ValueType::Rows(ColumnType::materialize(o)),
            ValueType::Rows(r) => ValueType::Rows(ColumnType::materialize(r)),
            ValueType::Row(r) => ValueType::Row(ColumnType::materialize(r)),
            ValueType::List(l) => ValueType::List(Box::from(l.materialize())),
            ValueType::Dict(k, v) => ValueType::Dict(Box::from(k.materialize()), Box::from(v.materialize())),
        }
    }

        pub fn is_hashable(&self) -> bool {
        match self {
            ValueType::Env | ValueType::Closure | ValueType::List(_) | ValueType::Dict(_, _) | ValueType::Output(_) | ValueType::Rows(_) => false,
            _ => true,
        }
    }

    pub fn is_comparable(&self) -> bool {
        self.is_hashable()
    }

    pub fn to_string(&self) -> String {
        match self {
            ValueType::Text => "text".to_string(),
            ValueType::Integer => "integer".to_string(),
            ValueType::Time => "time".to_string(),
            ValueType::Duration => "duration".to_string(),
            ValueType::Field => "field".to_string(),
            ValueType::Glob => "glob".to_string(),
            ValueType::Regex => "regex".to_string(),
            ValueType::Op => "op".to_string(),
            ValueType::Command => "command".to_string(),
            ValueType::Closure => "closure".to_string(),
            ValueType::File => "file".to_string(),
            ValueType::Output(o) => format!("output<{}>", o.iter().map(|i| i.to_string()).collect::<Vec<String>>().join(",")),
            ValueType::Rows(r) => format!("rows<{}>", r.iter().map(|i| i.to_string()).collect::<Vec<String>>().join(",")),
            ValueType::Row(r) => format!("row<{}>", r.iter().map(|i| i.to_string()).collect::<Vec<String>>().join(",")),
            ValueType::List(l) => format!("list<{}>", l.to_string()),
            ValueType::Dict(k, v) => format!("dict<{},{}>", k.to_string(), v.to_string()),
            ValueType::Env => "env".to_string(),
            ValueType::Bool => "bool".to_string(),
        }
    }

    pub fn parse(&self, s: &str) -> JobResult<Value> {
        match self {
            ValueType::Text => Ok(Value::Text(Box::from(s))),
            ValueType::Integer => match s.parse::<i128>() {
                Ok(n) => Ok(Value::Integer(n)),
                Err(e) => Err(error(e.description())),
            }
            ValueType::Field => Ok(Value::Field(mandate(parse_name(s), "Invalid field name")?)),
            ValueType::Glob => Ok(Value::Glob(Glob::new(s))),
            ValueType::Regex => match Regex::new(s) {
                Ok(r) => Ok(Value::Regex(Box::from(s), r)),
                Err(e) => Err(error(e.description())),
            }
            ValueType::File => Ok(Value::Text(Box::from(s))),
            ValueType::Op => match s {
                "==" | "!=" | ">" | ">=" | "<" | "<=" | "=~" | "!~" => Ok(Value::Op(Box::from(s))),
                _ => Err(error("Invalid operator")),
            }
            _ => Err(error("Failed to parse cell")),
        }
    }
}
