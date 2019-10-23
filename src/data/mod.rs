mod cell;
mod row;
mod rows;
mod argument;

use crate::commands::{Exec};
use crate::errors::{JobError, error};
use std::fmt::Formatter;
use crate::stream::{InputStream, OutputStream};
use std::hash::Hasher;
use regex::Regex;
use std::error::Error;

pub use cell::Cell;
pub use cell::CellDefinition;
pub use cell::Alignment;
pub use argument::Argument;
pub use argument::BaseArgument;
pub use argument::ArgumentDefinition;
pub use row::Row;
pub use rows::Rows;
use crate::glob::Glob;

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
    Output(Vec<CellFnurp>),
    Rows(Vec<CellFnurp>),
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

#[derive(Clone)]
pub struct Command {
    pub call: fn(
        Vec<CellFnurp>,
        InputStream,
        OutputStream,
        Vec<Argument>,
    ) -> Result<(Exec, Vec<CellFnurp>), JobError>,
}

impl Command {
    pub fn new(call: fn(
        Vec<CellFnurp>,
        InputStream,
        OutputStream,
        Vec<Argument>,
    ) -> Result<(Exec, Vec<CellFnurp>), JobError>) -> Command {
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
pub struct CellFnurp {
    pub name: Option<Box<str>>,
    pub cell_type: CellType,
}

impl CellFnurp {
    pub fn named(name: &str, cell_type: CellType) -> CellFnurp {
        CellFnurp {
            name: Some(Box::from(name)),
            cell_type,
        }
    }

    pub fn len_or_0(&self) -> usize {
        self.name.as_ref().map(|v| v.len()).unwrap_or(0)
    }

    pub fn val_or_empty(&self) -> &str {
        self.name.as_ref().map(|v| v.as_ref()).unwrap_or("")
    }
}

#[derive(Debug)]
pub struct JobOutput {
    pub types: Vec<CellFnurp>,
    pub stream: InputStream,
}
