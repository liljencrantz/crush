use crate::data::{ColumnType, Row};
use std::hash::Hasher;
use crate::errors::JobError;

#[derive(Debug)]
#[derive(PartialEq)]
#[derive(PartialOrd)]
pub struct Rows {
    pub types: Vec<ColumnType>,
    pub rows: Vec<Row>,
}

impl Rows {
    pub fn partial_clone(&self) -> Result<Self, JobError> {
        Ok(Rows {
            types: self.types.clone(),
            rows: self.rows.iter().map(|r| r.partial_clone()).collect::<Result<Vec<Row>, JobError>>()?,
        })
    }
}
