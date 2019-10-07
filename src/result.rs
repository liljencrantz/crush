use std::cmp::Ordering;
use chrono::{Local, DateTime};

#[derive(Clone)]
#[derive(PartialEq)]
#[derive(Debug)]
pub enum CellDataType {
    Text,
    Integer,
    Time,
    Field,
    Wildcard,
    Regex,
    Op,
}

#[derive(Clone)]
#[derive(Debug)]
pub struct CellType {
    pub name: String,
    pub cell_type: CellDataType,
}

#[derive(Eq)]
#[derive(Clone)]
#[derive(Debug)]
pub enum Cell {
    Text(String),
    Integer(i128),
    Time(DateTime<Local>),
    Field(String),
    Wildcard(String),
    Regex(String),
    Op(String),
//    Float(f64),
//    Row(Box<Row>),
//    Rows(Vec<Row>),
}

impl Cell {
    pub fn cell_data_type(&self) -> CellDataType {
        return match self {
            Cell::Text(_) => CellDataType::Text,
            Cell::Integer(_) => CellDataType::Integer,
            Cell::Time(_) => CellDataType::Time,
            Cell::Field(_) => CellDataType::Field,
            Cell::Wildcard(_) => CellDataType::Wildcard,
            Cell::Regex(_) => CellDataType::Regex,
            Cell::Op(_) => CellDataType::Op,
        };
    }
}

impl std::cmp::PartialOrd for Cell {
    fn partial_cmp(&self, other: &Cell) -> Option<Ordering> {
        return match (self, other) {
            (Cell::Text(val1), Cell::Text(val2)) => Some(val1.cmp(val2)),
            (Cell::Field(val1), Cell::Field(val2)) => Some(val1.cmp(val2)),
            (Cell::Wildcard(val1), Cell::Wildcard(val2)) => Some(val1.cmp(val2)),
            (Cell::Regex(val1), Cell::Regex(val2)) => Some(val1.cmp(val2)),
            (Cell::Integer(val1), Cell::Integer(val2)) => Some(val1.cmp(val2)),
            (Cell::Time(val1), Cell::Time(val2)) => Some(val1.cmp(val2)),
            (Cell::Op(val1), Cell::Op(val2)) => Some(val1.cmp(val2)),
            _ => Option::None,
        };
    }
}

impl std::cmp::PartialEq for Cell {
    fn eq(&self, other: &Cell) -> bool {
        return match (self, other) {
            (Cell::Text(val1), Cell::Text(val2)) => val1 == val2,
            (Cell::Integer(val1), Cell::Integer(val2)) => val1 == val2,
            (Cell::Time(val1), Cell::Time(val2)) => val1 == val2,
            (Cell::Field(val1), Cell::Field(val2)) => val1 == val2,
            (Cell::Wildcard(val1), Cell::Wildcard(val2)) => val1 == val2,
            (Cell::Regex(val1), Cell::Regex(val2)) => val1 == val2,
            (Cell::Op(val1), Cell::Op(val2)) => val1 == val2,
            _ => false,
        };
    }
}

#[derive(Clone)]
pub struct Argument {
    pub name: String,
    pub cell: Cell,
}

impl Argument {
    pub fn named(name: &String, cell: &Cell) -> Argument {
        return Argument {
            name: name.clone(),
            cell: cell.clone(),
        };
    }

    pub fn unnamed(cell: &Cell) -> Argument {
        return Argument {
            name: String::from(""),
            cell: cell.clone(),
        };
    }
}

pub struct Row {
    pub cells: Vec<Cell>,
}
