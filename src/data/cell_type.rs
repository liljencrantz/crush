use crate::errors::{error, JobError};
use crate::data::{Cell, ColumnType};
use crate::glob::Glob;
use regex::Regex;
use std::error::Error;

#[derive(Clone)]
#[derive(PartialEq)]
#[derive(Debug)]
pub enum CellType {
    Text,
    Integer,
    Time,
    Field,
    Glob,
    Regex,
    Op,
    Command,
    Closure,
    File,
    Output(Vec<ColumnType>),
    Rows(Vec<ColumnType>),
    List(Box<CellType>),
}

impl CellType {
    pub fn from(s: &str) -> Result<CellType, JobError> {
        match s {
            "text" => Ok(CellType::Text),
            "integer" => Ok(CellType::Integer),
            "time" => Ok(CellType::Time),
            "field" => Ok(CellType::Field),
            "glob" => Ok(CellType::Glob),
            "regex" => Ok(CellType::Regex),
            "op" => Ok(CellType::Op),
            "command" => Ok(CellType::Command),
            "file" => Ok(CellType::File),
            _ => Err(error(format!("Unknown cell type {}", s).as_str())),
        }
    }

    pub fn parse(&self, s: &str) -> Result<Cell, JobError> {
        match self {
            CellType::Text => Ok(Cell::Text(Box::from(s))),
            CellType::Integer => match s.parse::<i128>() {
                Ok(n) => Ok(Cell::Integer(n)),
                Err(e) => Err(error(e.description())),
            }
            CellType::Field => Ok(Cell::Field(Box::from(s))),
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
