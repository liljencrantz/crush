use crate::data::{ColumnType, Row, Value};
use crate::errors::{CrushError, error};
use crate::stream::Readable;
use crate::replace::Replace;

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct Rows {
    types: Vec<ColumnType>,
    rows: Vec<Row>,
}

impl Rows {
    pub fn new(types: Vec<ColumnType>, rows: Vec<Row>) -> Rows {
        Rows {types, rows}
    }

    pub fn materialize(mut self) ->  Rows{
        Rows {
            types: ColumnType::materialize(&self.types),
            rows: self.rows.drain(..).map(|r| r.materialize()).collect(),
        }
    }

    pub fn types(&self) -> &Vec<ColumnType> {
        &self.types
    }

    pub fn rows(&self) -> &Vec<Row> {
        &self.rows
    }

    pub fn reader(self) -> RowsReader {
        RowsReader::new(self)
    }
}

pub struct RowsReader {
    idx: usize,
    rows: Rows,
    row_type: Vec<ColumnType>,
}

impl RowsReader {
    pub fn new(rows: Rows) -> RowsReader {
        RowsReader{
            idx: 0,
            row_type: rows.types().clone(),
            rows,
        }
    }
}

impl Readable for RowsReader {

    fn read(&mut self) -> Result<Row, CrushError> {
        if self.idx >= self.rows.rows().len() {
            return error("EOF");
        }
        self.idx += 1;
        return Ok(self.rows.rows.replace(self.idx - 1, Row::new(vec![Value::Integer(0)])));
    }

    fn types(&self) -> &Vec<ColumnType> {
        &self.row_type
    }
}
