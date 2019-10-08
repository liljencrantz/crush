use std::collections::VecDeque;
use crate::cell::{Row, Cell};
use crate::cell::CellType;
use std::cmp::max;
use std::sync::mpsc::{Receiver, sync_channel, SyncSender};

pub fn streams(row_type: &Vec<CellType>) -> (OutputStream, InputStream) {
    let (tx, rx): (SyncSender<Row>, Receiver<Row>) = sync_channel(200);
    return (
        OutputStream {
            sender: tx,
        },
        InputStream {
            receiver: rx,
            row_type: row_type.clone(),
        }
    );
}

pub struct OutputStream {
    sender: SyncSender<Row>,
}

impl OutputStream {
    pub fn add(&mut self, row: Row) {
        self.sender.send(row);
    }

    pub fn close(&mut self) {
    }
}

pub struct InputStream {
    receiver: Receiver<Row>,
    row_type: Vec<CellType>,
}

impl InputStream {
    pub fn next(&mut self) -> Option<Row> {
        return match self.receiver.recv() {
            Ok(res) => Some(res),
            Err(_) => None,
        };
    }

    pub fn get_row_type(&self) -> &Vec<CellType> {
        return &self.row_type;
    }
}
/*
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

    fn get_row_type(&self) -> &Vec<CellType> {
        return &self.row_type;
    }
}
*/

pub fn print(stream: &mut InputStream) {

    let mut data: Vec<Row> = Vec::new();
    loop {
        match stream.next() {
            Some(r) => data.push(r),
            None => break,
        }
    }
    let types = stream.get_row_type();
    let mut w = vec![0; types.len()];

    for (idx, val) in types.iter().enumerate() {
        w[idx] = max(w[idx], val.name.len());
    }

    for r in &data {
        assert_eq!(types.len(), r.cells.len());
        for (idx, c) in r.cells.iter().enumerate() {
            let l = match c {
                Cell::Text(val) => val.len(),
                Cell::Integer(val) => val.to_string().len(),
                Cell::Time(val) => val.format("%Y %b %d %H:%M:%S %z").to_string().len(),
                Cell::Field(val) => { val.len() + 3 }
                Cell::Glob(val) => { val.len() + 3 }
                Cell::Regex(val) => { val.len() + 3 }
                Cell::Op(val) => { val.len() }
            };
            w[idx] = max(w[idx], l);
        }
    }

    for (idx, val) in types.iter().enumerate() {
        print!("{}{}", val.name, " ".repeat(w[idx] - val.name.len() + 1))
    }
    println!();

    for r in &data {
        for (idx, c) in r.cells.iter().enumerate() {
            let cell = match c {
                Cell::Text(val) => String::from(val),
                Cell::Integer(val) => val.to_string(),
                Cell::Time(val) => val.format("%Y-%m-%d %H:%M:%S %z").to_string(),
                Cell::Field(val) => format!(r"%{{{}}}", val),
                Cell::Glob(val) => format!("*{{{}}}", val),
                Cell::Regex(val) => format!("r{{{}}}", val),
                Cell::Op(val) => String::from(val),
            };
            print!("{}{}", cell, " ".repeat(w[idx] - cell.len() + 1))
        }
        println!();
    }
}
