use crate::errors::{error, mandate, CrushResult, to_job_error};
use crate::lang::{Value, ColumnType, value_type_parser};
use crate::glob::Glob;
use regex::Regex;
use std::error::Error;
use crate::lang::parse_name;

#[derive(Clone)]
#[derive(PartialEq)]
#[derive(Eq)]
#[derive(PartialOrd)]
#[derive(Ord)]
#[derive(Debug)]
#[derive(Hash)]
pub enum ValueType {
    Text,
    Integer,
    Time,
    Duration,
    Field,
    Glob,
    Regex,
    Command,
    Closure,
    File,
    Stream(Vec<ColumnType>),
    Rows(Vec<ColumnType>),
    Row(Vec<ColumnType>),
    List(Box<ValueType>),
    Dict(Box<ValueType>, Box<ValueType>),
    Env,
    Bool,
    Float,
    Empty,
    Any,
    BinaryStream,
    Binary,
    Type,
}



impl ValueType {
    pub fn from(s: &str) -> CrushResult<ValueType> {
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
            ValueType::Command |
            ValueType::Closure |
            ValueType::File |
            ValueType::Env |
            ValueType::Float |
            ValueType::Empty |
            ValueType::Any |
            ValueType::Binary |
            ValueType::Type |
            ValueType::Bool => self.clone(),
            ValueType::BinaryStream => ValueType::Binary,
            ValueType::Stream(o) => ValueType::Rows(ColumnType::materialize(o)),
            ValueType::Rows(r) => ValueType::Rows(ColumnType::materialize(r)),
            ValueType::Row(r) => ValueType::Row(ColumnType::materialize(r)),
            ValueType::List(l) => ValueType::List(Box::from(l.materialize())),
            ValueType::Dict(k, v) => ValueType::Dict(Box::from(k.materialize()), Box::from(v.materialize())),
        }
    }

        pub fn is_hashable(&self) -> bool {
        match self {
            ValueType::Env | ValueType::Closure | ValueType::List(_) | ValueType::Dict(_, _) | ValueType::Stream(_) | ValueType::Rows(_) => false,
            _ => true,
        }
    }

    pub fn is_comparable(&self) -> bool {
        self.is_hashable()
    }

    pub fn parse(&self, s: &str) -> CrushResult<Value> {
        match self {
            ValueType::Text => Ok(Value::Text(Box::from(s))),
            ValueType::Integer => match s.parse::<i128>() {
                Ok(n) => Ok(Value::Integer(n)),
                Err(e) => error(e.description()),
            }
            ValueType::Field => Ok(Value::Field(mandate(parse_name(s), "Invalid field name")?)),
            ValueType::Glob => Ok(Value::Glob(Glob::new(s))),
            ValueType::Regex => Ok(Value::Regex(Box::from(s), to_job_error(Regex::new(s))?)),
            ValueType::File => Ok(Value::Text(Box::from(s))),
            ValueType::Float => Ok(Value::Float(to_job_error(s.parse::<f64>())?)),
            ValueType::Type => Ok(Value::Type(value_type_parser::parse(s)?)),
            ValueType::Bool => Ok(Value::Bool(to_job_error(s.parse::<bool>())?)),
            _ => error("Failed to parse cell"),
        }
    }
}

impl ToString for ValueType {
    fn to_string(&self) -> String {
        match self {
            ValueType::Text => "text".to_string(),
            ValueType::Integer => "integer".to_string(),
            ValueType::Time => "time".to_string(),
            ValueType::Duration => "duration".to_string(),
            ValueType::Field => "field".to_string(),
            ValueType::Glob => "glob".to_string(),
            ValueType::Regex => "regex".to_string(),
            ValueType::Command => "command".to_string(),
            ValueType::Closure => "closure".to_string(),
            ValueType::File => "file".to_string(),
            ValueType::Stream(o) => format!("output<{}>", o.iter().map(|i| i.to_string()).collect::<Vec<String>>().join(",")),
            ValueType::Rows(r) => format!("rows<{}>", r.iter().map(|i| i.to_string()).collect::<Vec<String>>().join(",")),
            ValueType::Row(r) => format!("row<{}>", r.iter().map(|i| i.to_string()).collect::<Vec<String>>().join(",")),
            ValueType::List(l) => format!("list<{}>", l.to_string()),
            ValueType::Dict(k, v) => format!("dict<{},{}>", k.to_string(), v.to_string()),
            ValueType::Env => "env".to_string(),
            ValueType::Bool => "bool".to_string(),
            ValueType::Float => "float".to_string(),
            ValueType::Empty => "empty".to_string(),
            ValueType::Any => "any".to_string(),
            ValueType::BinaryStream => "binary_reader".to_string(),
            ValueType::Binary => "binary".to_string(),
            ValueType::Type => "type".to_string(),
        }
    }
}
