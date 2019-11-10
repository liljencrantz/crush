use crate::data::cell::Cell;
use crate::errors::{JobResult};
use crate::data::ColumnType;
use std::mem;

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

    pub fn materialize(mut self) ->  Row{
        Row {
            cells: self.cells.drain(..).map(|c| c.materialize()).collect(),
        }
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

    pub fn get(mut self, name: &str) -> Option<Cell> {
        for (idx, t) in self.types.iter().enumerate() {
            match &t.name {
                None => {},
                Some(n) => if n.as_ref() == name {return Some(mem::replace(&mut self.cells[idx], Cell::Integer(0)));},
            }
        }
        None
    }

    pub fn materialize(mut self) ->  RowWithTypes {
        RowWithTypes {
            types: self.types,
            cells: self.cells.drain(..).map(|c| c.materialize()).collect(),
        }
    }
}

impl ToString for RowWithTypes {
    fn to_string(&self) -> String {
        self.cells.iter().map(|c| c.to_string()).collect::<Vec<String>>().join(", ")
    }
}

