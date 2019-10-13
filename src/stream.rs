use crate::cell::{Row, Cell, Alignment};
use crate::cell::CellType;
use std::cmp::max;
use std::sync::mpsc::{Receiver, sync_channel, SyncSender};

pub type OutputStream = SyncSender<Row>;
pub type InputStream = Receiver<Row>;

pub fn streams() -> (OutputStream, InputStream) {
    return sync_channel(200);
}

pub fn print(stream: InputStream, types: Vec<CellType>) {

    let mut data: Vec<Row> = Vec::new();
    loop {
        match stream.recv() {
            Ok(r) => data.push(r),
            Err(_) => break,
        }
        if data.len() == 49 {
            print_partial(&mut data, &types);
        }
    }
    if !data.is_empty() {
        print_partial(&mut data, &types);
    }
}

pub fn print_partial(data: &mut Vec<Row>, types: &Vec<CellType>) {

    let mut w = vec![0; types.len()];

    for (idx, val) in types.iter().enumerate() {
        w[idx] = max(w[idx], val.name.len());
    }

    for r in data.into_iter() {
        assert_eq!(types.len(), r.cells.len());
        for (idx, c) in r.cells.iter().enumerate() {
            let l = c.to_string().len();
            w[idx] = max(w[idx], l);
        }
    }

    for (idx, val) in types.iter().enumerate() {
        print!("{}{}", val.name, " ".repeat(w[idx] - val.name.len() + 1))
    }
    println!();

    for r in data.into_iter() {
        for (idx, c) in r.cells.iter().enumerate() {
            let cell = c.to_string();
            let spaces = if idx == r.cells.len()-1 {"".to_owned()} else {" ".repeat(w[idx] - cell.len())};
            match c.alignment() {
                Alignment::Right => print!("{}{} ", spaces, cell),
                _ => print!("{}{} ", cell, spaces),
            }
        }
        println!();
    }
    data.drain(..);
}
