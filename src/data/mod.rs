pub mod cell;

use crate::commands::Call;
use crate::errors::{JobError};
use std::fmt::Formatter;
use crate::stream::InputStream;
use std::hash::Hasher;
use regex::Regex;
use crate::data::cell::Cell;

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
}

impl CellDataType {
    pub fn from(s: &str) -> CellDataType {
        match s {
            "text" => CellDataType::Text,
            "integer" => CellDataType::Integer,
            "time" => CellDataType::Time,
            "field" => CellDataType::Field,
            _ => panic!(format!("Missing conversion for {} in CellDataType", s)),
        }
    }

    pub fn parse(&self, s: &str) -> Result<Cell, JobError> {
        match self {
            CellDataType::Text => Ok(Cell::Text(s.to_string())),
            CellDataType::Integer => Ok(Cell::Integer(s.parse::<i128>().unwrap())),
            CellDataType::Field => Ok(Cell::Field(s.to_string())),
            CellDataType::Glob => Ok(Cell::Glob(s.to_string())),
            CellDataType::Regex => Ok(Cell::Regex(s.to_string(), Regex::new(s).unwrap())),
            CellDataType::File => Ok(Cell::Text(s.to_string())),
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

#[derive(Debug)]
pub struct Rows {
    pub types: Vec<CellType>,
    pub rows: Vec<Row>,
}

impl std::hash::Hash for Rows {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for r in &self.rows {
            r.hash(state);
        }
    }
}


impl Clone for Rows {
    fn clone(&self) -> Self {
        Rows {
            types: self.types.clone(),
            rows: self.rows.iter().map(|r| r.concrete()).collect(),
        }
    }
}


pub struct Argument {
    pub name: String,
    pub cell: Cell,
}

impl Argument {
    pub fn named(name: &String, cell: Cell) -> Argument {
        return Argument {
            name: name.clone(),
            cell: cell,
        };
    }

    pub fn unnamed(cell: Cell) -> Argument {
        return Argument {
            name: String::from(""),
            cell: cell,
        };
    }
}

#[derive(Debug)]
pub struct Row {
    pub cells: Vec<Cell>,
}

impl Row {
    pub fn concrete(&self) -> Self {
        Row {cells: self.cells.iter().map(|c| c.concrete()).collect()}
    }
}

impl std::hash::Hash for Row {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for c in &self.cells {
            c.hash(state);
        }
    }
}
