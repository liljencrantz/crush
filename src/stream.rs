use std::collections::VecDeque;
use crate::result::Row;
use crate::result::CellType;

pub struct Stream {
    pub row_type: Vec<CellType>,
    pub closed: bool,
    pub data: VecDeque<Row>
}

impl Stream {
    pub fn new(row_type: Vec<CellType>) -> Stream {
        return Stream {
            row_type,
            closed: false,
            data: VecDeque::new(),
        }
    }

    pub fn next(&mut self) -> Option<Row> {
        return None;
    }

    pub fn add(&mut self, row: Row) {
        self.data.push_back(row)
    }

    fn close(&mut self) {
        self.closed = true;
    }
}
