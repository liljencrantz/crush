use crate::lang::{column_type::ColumnType, row::Row, value::Value};
use crate::errors::{CrushError, error, CrushResult};
use crate::stream::{Readable, InputStream};
use crate::replace::Replace;

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct Table {
    types: Vec<ColumnType>,
    rows: Vec<Row>,
}

impl Table {
    pub fn new(types: Vec<ColumnType>, rows: Vec<Row>) -> Table {
        Table {types, rows}
    }

    pub fn materialize(mut self) -> Table {
        Table {
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

    pub fn reader(self) -> TableReader {
        TableReader::new(self)
    }
}

pub struct TableReader {
    idx: usize,
    rows: Table,
    row_type: Vec<ColumnType>,
}

impl TableReader {
    pub fn new(rows: Table) -> TableReader {
        TableReader {
            idx: 0,
            row_type: rows.types().clone(),
            rows,
        }
    }
}

impl Readable for TableReader {

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

#[derive(Debug, Clone)]
pub struct TableStream {
    pub stream: InputStream,
}

impl TableStream {
    pub fn get(&self, idx: i128) -> CrushResult<Row> {
        let mut i = 0i128;
        loop {
            match self.stream.recv() {
                Ok(row) => {
                    if i == idx {
                        return Ok(row);
                    }
                    i += 1;
                },
                Err(_) => return error("Index out of bounds"),
            }
        }
    }

}
