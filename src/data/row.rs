use crate::data::cell::Cell;
use std::hash::Hasher;
use crate::errors::JobError;

#[derive(PartialEq)]
#[derive(PartialOrd)]
#[derive(Debug)]
pub struct Row {
    pub cells: Vec<Cell>,
}

impl Row {
    pub fn partial_clone(&self) -> Result<Self, JobError> {
        Ok(Row {
            cells: self.cells.iter().map(|c| c.partial_clone()).collect::<Result<Vec<Cell>, JobError>>()?,
        })
    }
}
