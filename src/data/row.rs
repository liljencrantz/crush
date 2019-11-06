use crate::data::cell::Cell;
use std::hash::Hasher;
use crate::errors::{JobResult};
use crate::data::ColumnType;

#[derive(PartialEq)]
#[derive(PartialOrd)]
#[derive(Debug)]
pub struct Row {
    pub cells: Vec<Cell>,
}

impl Row {
    pub fn partial_clone(&self) -> JobResult<Self> {
        Ok(Row {
            cells: self.cells.iter().map(|c| c.partial_clone()).collect::<JobResult<Vec<Cell>>>()?,
        })
    }
}

#[derive(PartialEq)]
#[derive(PartialOrd)]
#[derive(Debug)]
pub struct RowWithTypes {
    pub types: Vec<ColumnType>,
    pub cells: Vec<Cell>,
}

impl RowWithTypes {
    pub fn partial_clone(&self) -> JobResult<Self> {
        Ok(RowWithTypes {
            types: self.types.clone(),
            cells: self.cells.iter().map(|c| c.partial_clone()).collect::<JobResult<Vec<Cell>>>()?,
        })
    }
}

impl ToString for RowWithTypes {
    fn to_string(&self) -> String {
        self.cells.iter().map(|c| c.to_string()).collect::<Vec<String>>().join(", ")
    }
}

