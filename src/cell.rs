use std::cmp::Ordering;
use chrono::{Local, DateTime};
use crate::glob::glob;
use crate::commands::Call;
use crate::errors::{JobError, error};
use std::fmt::{Formatter};
use std::path::Path;
use crate::stream::InputStream;
use std::hash::Hasher;

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
    Output,
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
pub enum Cell {
    Text(String),
    Integer(i128),
    Time(DateTime<Local>),
    Field(String),
    Glob(String),
    Regex(String),
    Op(String),
    Command(Command),
    //    Float(f64),
//    Row(Box<Row>),
//    Rows(Vec<Row>),
    Output(Output),
    File(Box<Path>),
}

impl Cell {
    pub fn cell_data_type(&self) -> CellDataType {
        return match self {
            Cell::Text(_) => CellDataType::Text,
            Cell::Integer(_) => CellDataType::Integer,
            Cell::Time(_) => CellDataType::Time,
            Cell::Field(_) => CellDataType::Field,
            Cell::Glob(_) => CellDataType::Glob,
            Cell::Regex(_) => CellDataType::Regex,
            Cell::Op(_) => CellDataType::Op,
            Cell::Command(_) => CellDataType::Command,
            Cell::File(_) => CellDataType::File,
            Cell::Output(_) => CellDataType::Output,
        };
    }

    pub fn partial_clone(&self) -> Result<Cell, JobError> {
        return match self {
            Cell::Text(v) => Ok(Cell::Text(v.clone())),
            Cell::Integer(v) => Ok(Cell::Integer(v.clone())),
            Cell::Time(v) => Ok(Cell::Time(v.clone())),
            Cell::Field(v) => Ok(Cell::Field(v.clone())),
            Cell::Glob(v) => Ok(Cell::Glob(v.clone())),
            Cell::Regex(v) => Ok(Cell::Regex(v.clone())),
            Cell::Op(v) => Ok(Cell::Op(v.clone())),
            Cell::Command(v) => Ok(Cell::Command(v.clone())),
            Cell::File(v) => Ok(Cell::File(v.clone())),
            Cell::Output(_) => Err(error("Invalid use of stream")),
        };
    }

    pub fn concrete(&self) -> Cell {
        return match self {
            Cell::Text(v) => Cell::Text(v.clone()),
            Cell::Integer(v) => Cell::Integer(v.clone()),
            Cell::Time(v) => Cell::Time(v.clone()),
            Cell::Field(v) => Cell::Field(v.clone()),
            Cell::Glob(v) => Cell::Glob(v.clone()),
            Cell::Regex(v) => Cell::Regex(v.clone()),
            Cell::Op(v) => Cell::Op(v.clone()),
            Cell::Command(v) => Cell::Command(v.clone()),
            Cell::File(v) => Cell::File(v.clone()),
            Cell::Output(_) => panic!("UNIMPLEMENTED!!!!"),
        };
    }

    pub fn to_string(&self) -> String {
        return match self {
            Cell::Text(val) => String::from(val),
            Cell::Integer(val) => val.to_string(),
            Cell::Time(val) => val.format("%Y-%m-%d %H:%M:%S %z").to_string(),
            Cell::Field(val) => format!(r"%{{{}}}", val),
            Cell::Glob(val) => format!("*{{{}}}", val),
            Cell::Regex(val) => format!("r{{{}}}", val),
            Cell::Op(val) => String::from(val),
            Cell::Command(_) => "Command".to_string(),
            Cell::File(val) => val.to_str().unwrap_or("<Broken file>").to_string(),
            Cell::Output(_) => "<Stream>".to_string(),
        };
    }

    pub fn alignment(&self) -> Alignment {
        return match self {
            Cell::Integer(_) => Alignment::Right,
            _ => Alignment::Left,
        };
    }
}


impl std::hash::Hash for Cell {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Cell::Text(v) => v.hash(state),
            Cell::Integer(v) => v.hash(state),
            Cell::Time(v) => v.hash(state),
            Cell::Field(v) => v.hash(state),
            Cell::Glob(v) => v.hash(state),
            Cell::Regex(v) => v.hash(state),
            Cell::Op(v) => v.hash(state),
            Cell::Command(_) => {panic!("Impossible!")},
            Cell::Output(_) => {panic!("Impossible!")},
            Cell::File(v) => v.hash(state),
        }
    }
}

pub enum Alignment {
    Left,
    Right,
}

impl std::cmp::PartialOrd for Cell {
    fn partial_cmp(&self, other: &Cell) -> Option<Ordering> {
        return match (self, other) {
            (Cell::Text(val1), Cell::Text(val2)) => Some(val1.cmp(val2)),
            (Cell::Field(val1), Cell::Field(val2)) => Some(val1.cmp(val2)),
            (Cell::Glob(val1), Cell::Glob(val2)) => Some(val1.cmp(val2)),
            (Cell::Regex(val1), Cell::Regex(val2)) => Some(val1.cmp(val2)),
            (Cell::Integer(val1), Cell::Integer(val2)) => Some(val1.cmp(val2)),
            (Cell::Time(val1), Cell::Time(val2)) => Some(val1.cmp(val2)),
            (Cell::Op(val1), Cell::Op(val2)) => Some(val1.cmp(val2)),
            (Cell::File(val1), Cell::File(val2)) => Some(val1.cmp(val2)),
            _ => Option::None,
        };
    }
}

impl std::cmp::PartialEq for Cell {
    fn eq(&self, other: &Cell) -> bool {
        return match (self, other) {
            (Cell::Text(val1), Cell::Text(val2)) => val1 == val2,
            (Cell::Glob(glb), Cell::Text(val)) => glob(glb.as_str(), val.as_str()),
            (Cell::Text(val), Cell::Glob(glb)) => glob(glb.as_str(), val.as_str()),
            (Cell::Integer(val1), Cell::Integer(val2)) => val1 == val2,
            (Cell::Time(val1), Cell::Time(val2)) => val1 == val2,
            (Cell::Field(val1), Cell::Field(val2)) => val1 == val2,
            (Cell::Glob(val1), Cell::Glob(val2)) => val1 == val2,
            (Cell::Regex(val1), Cell::Regex(val2)) => val1 == val2,
            (Cell::Op(val1), Cell::Op(val2)) => val1 == val2,
            (Cell::Command(val1), Cell::Command(val2)) => val1 == val2,
            (Cell::File(val1), Cell::File(val2)) => val1 == val2,
            _ => false,
        };
    }
}

impl std::cmp::Eq for Cell {}

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

pub struct Row {
    pub cells: Vec<Cell>,
}
