use crate::cell::{Row, Cell, Alignment, Output};
use crate::cell::CellType;
use std::cmp::max;
use std::sync::mpsc::{Receiver, sync_channel, SyncSender};
use std::borrow::BorrowMut;

pub type OutputStream = SyncSender<Row>;
pub type InputStream = Receiver<Row>;

pub fn streams() -> (OutputStream, InputStream) {
    return sync_channel(200);
}

pub fn print(mut stream: InputStream, types: Vec<CellType>) {
    print_internal(&mut stream, &types, 0);
}

pub fn print_internal(stream: &InputStream, types: &Vec<CellType>, indent: usize) {
    let mut data: Vec<Row> = Vec::new();
    let mut has_name = false;
    for (idx, val) in types.iter().enumerate() {
        if val.name.len() > 0 {
            has_name = true;
            break;
        }
    }
    loop {
        match stream.recv() {
            Ok(r) => data.push(r),
            Err(_) => break,
        }
        if data.len() == 49 {
            print_partial(&mut data, &types, has_name, indent);
        }
    }
    if !data.is_empty() {
        print_partial(&mut data, &types, has_name, indent);
    }
}

pub fn print_partial(data: &Vec<Row>, types: &Vec<CellType>, has_name: bool, indent: usize) {
    let mut w = vec![0; types.len()];

    if (has_name) {
        for (idx, val) in types.iter().enumerate() {
            w[idx] = max(w[idx], val.name.len());
        }
    }

    for r in data.into_iter() {
        assert_eq!(types.len(), r.cells.len());
        for (idx, c) in r.cells.iter().enumerate() {
            let l = c.to_string().len();
            w[idx] = max(w[idx], l);
        }
    }

    if has_name {
        print!("{}", " ".repeat(indent*4));
        for (idx, val) in types.iter().enumerate() {
            print!("{}{}", val.name, " ".repeat(w[idx] - val.name.len() + 1))
        }
        println!();
    }

    for r in data.into_iter() {
        let mut outputs: Vec<&Output> = Vec::new();
        for (idx, c) in r.cells.iter().enumerate() {
            if let Cell::Output(o) = c {
                outputs.push(o);
            }

            let cell = c.to_string();
            let spaces = if idx == r.cells.len() - 1 { "".to_owned() } else { " ".repeat(w[idx] - cell.len()) };
            print!("{}", " ".repeat(indent*4));
            match c.alignment() {
                Alignment::Right => print!("{}{} ", spaces, cell),
                _ => print!("{}{} ", cell, spaces),
            }
        }
        println!();

        for o in outputs {
            print_internal(&o.stream, &o.types, indent+1);
        }

    }
}
