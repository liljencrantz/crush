use crate::errors::{error, mandate, JobResult};
use crate::data::{Cell, ColumnType, cell_type_parser};
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
pub enum CellType {
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
    List(Box<CellType>),
    Dict(Box<CellType>, Box<CellType>),
    Env,
    Bool,
}

impl CellType {
    pub fn from(s: &str) -> JobResult<CellType> {
        cell_type_parser::parse(s)
    }

    pub fn is_hashable(&self) -> bool {
        return match self {
            CellType::Env | CellType::Closure | CellType::List(_) | CellType::Dict(_, _) | CellType::Output(_) | CellType::Rows(_) => false,
            _ => true,
        };
    }

    pub fn to_string(&self) -> String {
        match self {
            CellType::Text => "text".to_string(),
            CellType::Integer => "integer".to_string(),
            CellType::Time => "time".to_string(),
            CellType::Duration => "duration".to_string(),
            CellType::Field => "field".to_string(),
            CellType::Glob => "glob".to_string(),
            CellType::Regex => "regex".to_string(),
            CellType::Op => "op".to_string(),
            CellType::Command => "command".to_string(),
            CellType::Closure => "closure".to_string(),
            CellType::File => "file".to_string(),
            CellType::Output(o) => format!("output<{}>", o.iter().map(|i| i.to_string()).collect::<Vec<String>>().join(",")),
            CellType::Rows(r) => format!("rows<{}>", r.iter().map(|i| i.to_string()).collect::<Vec<String>>().join(",")),
            CellType::Row(r) => format!("row<{}>", r.iter().map(|i| i.to_string()).collect::<Vec<String>>().join(",")),
            CellType::List(l) => format!("list<{}>", l.to_string()),
            CellType::Dict(k, v) => format!("dict<{},{}>", k.to_string(), v.to_string()),
            CellType::Env => "env".to_string(),
            CellType::Bool => "bool".to_string(),
        }
    }

    pub fn parse(&self, s: &str) -> JobResult<Cell> {
        match self {
            CellType::Text => Ok(Cell::Text(Box::from(s))),
            CellType::Integer => match s.parse::<i128>() {
                Ok(n) => Ok(Cell::Integer(n)),
                Err(e) => Err(error(e.description())),
            }
            CellType::Field => Ok(Cell::Field(mandate(parse_name(s), "Invalid field name")?)),
            CellType::Glob => Ok(Cell::Glob(Glob::new(s))),
            CellType::Regex => match Regex::new(s) {
                Ok(r) => Ok(Cell::Regex(Box::from(s), r)),
                Err(e) => Err(error(e.description())),
            }
            CellType::File => Ok(Cell::Text(Box::from(s))),
            CellType::Op => match s {
                "==" | "!=" | ">" | ">=" | "<" | "<=" | "=~" | "!~" => Ok(Cell::Op(Box::from(s))),
                _ => Err(error("Invalid operator")),
            }
            _ => Err(error("Failed to parse cell")),
        }
    }
}
