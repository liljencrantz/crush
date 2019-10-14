use crate::cell::{Row, Cell, Alignment, Output, CellDataType};
use crate::cell::CellType;
use std::cmp::max;
use std::sync::mpsc::{Receiver, sync_channel, SyncSender, channel, Sender};
use crate::errors::{JobError, error};

pub enum OutputStream {
    Sync(SyncSender<Row>),
    Async(Sender<Row>),
}

impl OutputStream {
    pub fn send(&self, row: Row) -> Result<(), JobError> {
        let native_output = match self {
            OutputStream::Sync(s) => s.send(row),
            OutputStream::Async(s) => s.send(row),
        };
        return match native_output {
            Ok(_) => Ok(()),
            Err(_) => Err(error("Broken pipe")),
        }
    }
}

pub type InputStream = Receiver<Row>;

pub fn streams() -> (OutputStream, InputStream) {
    let temp = sync_channel(200000);
    return (OutputStream::Sync(temp.0), temp.1);
}
pub fn unlimited_streams() -> (OutputStream, InputStream) {
    let temp = channel();
    return (OutputStream::Async(temp.0), temp.1);
}

pub fn print(mut stream: InputStream, types: Vec<CellType>) {
    print_internal(&mut stream, &types, 0);
}

pub fn print_internal(stream: &InputStream, types: &Vec<CellType>, indent: usize) {
    let mut data: Vec<Row> = Vec::new();
    let mut has_name = false;
    let mut has_stream = false;

    for val in types.iter() {
        match val.cell_type {
            CellDataType::Output => has_stream = true,
            _ => (),
        }
        if val.name.len() > 0 {
            has_name = true;
        }
    }
    loop {
        match stream.recv() {
            Ok(r) => {
                data.push(r)
            }
            Err(_) => break,
        }
        if data.len() == 49 || has_stream {
            print_partial(&mut data, &types, has_name, indent);
            data.drain(..);
        }
    }
    if !data.is_empty() {
        print_partial(&mut data, &types, has_name, indent);
    }
}

pub fn print_partial(data: &Vec<Row>, types: &Vec<CellType>, has_name: bool, indent: usize) {
    let mut w = vec![0; types.len()];

    if has_name {
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
        print!("{}", " ".repeat(indent * 4));
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
            print!("{}", " ".repeat(indent * 4));
            match c.alignment() {
                Alignment::Right => print!("{}{} ", spaces, cell),
                _ => print!("{}{} ", cell, spaces),
            }
        }
        println!();

        for o in outputs {
            print_internal(&o.stream, &o.types, indent + 1);
        }
    }
}
