use std::collections::VecDeque;
use crate::result::{Row, Cell};
use crate::result::CellType;
use std::cmp::max;

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

    pub fn print(&self, types: &Vec<CellType>) {
        let mut w = vec![0; types.len()];

        for (idx, val) in types.iter().enumerate() {
            w[idx] = max(w[idx], val.name.len());
        }

        for r in &self.data {
            assert!(types.len() == r.cells.len());
            for (idx, c) in r.cells.iter().enumerate() {
                let l = match c {
                    Cell::Text(val) => val.len(),
                    Cell::Integer(val) => val.to_string().len(),
                    Cell::Time(val) => val.format("%Y %b %d %H:%M:%S %z").to_string().len(),
                };
                w[idx] = max(w[idx], l);
            }
        }

        for (idx, val) in types.iter().enumerate() {
            print!("{}{}", val.name, " ".repeat(w[idx] - val.name.len() + 1))
        }
        println!();

        for r in &self.data {
            for (idx, c) in r.cells.iter().enumerate() {
                let cell = match c {
                    Cell::Text(val) => String::from(val),
                    Cell::Integer(val) => val.to_string(),
                    Cell::Time(val) => val.format("%Y-%m-%d %H:%M:%S %z").to_string(),
                };
                print!("{}{}", cell, " ".repeat(w[idx] - cell.len() + 1))
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
