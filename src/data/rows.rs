use crate::data::{ColumnType, Row};
use crate::errors::JobError;
use crate::stream::RowsReader;
use map_in_place::MapVecInPlace;

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

    pub fn materialize(mut self) ->  Rows{
        Rows {
            types: self.types,
            rows: self.rows.drain(..).map(|r| r.materialize()).collect(),
        }
    }

    pub fn reader(self) -> RowsReader {
        RowsReader::new(self)
    }
}
