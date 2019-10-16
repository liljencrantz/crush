mod cell;
mod row;
mod rows;
mod argument;

use crate::commands::Call;
use crate::errors::{JobError, error};
use std::fmt::Formatter;
use crate::stream::InputStream;
use std::hash::Hasher;
use regex::Regex;
use std::error::Error;

pub use cell::Cell;
pub use cell::Alignment;
pub use argument::Argument;
pub use row::Row;
pub use row::RowWithTypes;
pub use rows::Rows;

#[derive(Clone)]
#[derive(PartialEq)]
#[derive(Debug)]
pub enum CellDataType {
    Text,
    Integer,
    Time,
    Field,
    Glob,
    Regex,
    Op,
    Command,
    File,
    Output(Vec<CellType>),
    Rows(Vec<CellType>),
    Row(Vec<CellType>),
}

impl CellDataType {
    pub fn from(s: &str) -> CellDataType {
        match s {
            "text" => CellDataType::Text,
            "integer" => CellDataType::Integer,
            "time" => CellDataType::Time,
            "field" => CellDataType::Field,
            "glob" => CellDataType::Glob,
            "regex" => CellDataType::Regex,
            "op" => CellDataType::Op,
            "command" => CellDataType::Command,
            "file" => CellDataType::Command,
            _ => panic!(format!("Missing conversion for {} in CellDataType", s)),
        }
    }

    pub fn parse(&self, s: &str) -> Result<Cell, JobError> {
        match self {
            CellDataType::Text => Ok(Cell::Text(s.to_string())),
            CellDataType::Integer => Ok(Cell::Integer(s.parse::<i128>().unwrap())),
            CellDataType::Field => Ok(Cell::Field(s.to_string())),
            CellDataType::Glob => Ok(Cell::Glob(s.to_string())),
            CellDataType::Regex => match Regex::new(s) {
                Ok(r) => Ok(Cell::Regex(s.to_string(), r)),
                Err(e) => Err(error(e.description())),
            }
            CellDataType::File => Ok(Cell::Text(s.to_string())),
            CellDataType::Op => match s {
                "==" | "!=" | ">" | ">=" | "<" | "<=" | "=~" | "!~"=> Ok(Cell::Op(s.to_string())),
                _ => Err(error("Invalid operator")),
            }
            _ => panic!("AAAA"),
        }
    }

}

#[derive(Clone)]
pub struct Command {
    pub call: fn(Vec<CellType>, Vec<Argument>) -> Result<Call, JobError>,
}

impl Command {
    pub fn new(call: fn(Vec<CellType>, Vec<Argument>) -> Result<Call, JobError>) -> Command {
        return Command { call };
    }
}

impl std::cmp::PartialEq for Command {
    fn eq(&self, _other: &Command) -> bool {
        return false;
    }
}

impl std::cmp::Eq for Command {}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Command")
    }
}

#[derive(Clone)]
#[derive(Debug)]
#[derive(PartialEq)]
pub struct CellType {
    pub name: String,
    pub cell_type: CellDataType,
}

#[derive(Debug)]
pub struct Output {
    pub types: Vec<CellType>,
    pub stream: InputStream,
}

