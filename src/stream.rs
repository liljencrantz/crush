use std::collections::VecDeque;
use crate::cell::{Row, Cell};
use crate::cell::CellType;
use std::cmp::max;
use std::sync::mpsc::{Receiver, sync_channel, SyncSender};

pub type OutputStream = SyncSender<Row>;
pub type InputStream = Receiver<Row>;

pub fn streams() -> (OutputStream, InputStream) {
    let res: (SyncSender<Row>, Receiver<Row>) = sync_channel(200);
    return res;
}

pub fn print(stream: &mut InputStream, types: &Vec<CellType>) {

    let mut data: Vec<Row> = Vec::new();
    loop {
        match stream.recv() {
            Ok(r) => data.push(r),
            Err(_) => break,
        }
    }
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
