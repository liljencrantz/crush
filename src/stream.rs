use std::collections::VecDeque;
use crate::result::{Row, Cell};
use crate::result::CellType;

pub trait InputStream {
    fn next(&mut self) -> Option<Row>;
}

pub trait OutputStream {
    fn add(&mut self, row: Row);
    fn close(&mut self);
}

pub struct SerialStream {
    pub row_type: Vec<CellType>,
    pub closed: bool,
    pub data: VecDeque<Row>,
}

impl SerialStream {
    pub fn new(row_type: Vec<CellType>) -> SerialStream {
        return SerialStream {
            row_type,
            closed: false,
            data: VecDeque::new(),
        };
    }

    pub fn reset(&mut self) {
        self.data.clear();
    }

    pub fn print(&self) {
        for r in &self.data {
            for c in &r.cells {
                print!("{}",
                       match c {
                           Cell::String(val) => String::from(val),
                           Cell::Integer(val) => val.to_string(),
                       }
                );
            }
            println!();
        }
    }
}

impl OutputStream for SerialStream {
    fn add(&mut self, row: Row) {
        self.data.push_back(row)
    }

    fn close(&mut self) {
        self.closed = true;
    }
}

impl InputStream for SerialStream {
    fn next(&mut self) -> Option<Row> {
        return self.data.pop_front();
    }
}
