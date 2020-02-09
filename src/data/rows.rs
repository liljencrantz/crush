use crate::data::{ColumnType, Row};
use crate::errors::JobError;
use crate::stream::RowsReader;

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct Rows {
    pub types: Vec<ColumnType>,
    pub rows: Vec<Row>,
}

impl Rows {
    pub fn materialize(mut self) ->  Rows{
        Rows {
            types: ColumnType::materialize(&self.types),
            rows: self.rows.drain(..).map(|r| r.materialize()).collect(),
        }
    }

    pub fn get_type(&self) -> &Vec<ColumnType> {
        &self.types
    }

    pub fn reader(self) -> RowsReader {
        RowsReader::new(self)
    }
}
