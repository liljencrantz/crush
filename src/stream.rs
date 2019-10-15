use crate::cell::{Row, Cell, Alignment, Output, CellDataType, Rows};
use crate::cell::CellType;
use std::cmp::max;
use std::sync::mpsc::{Receiver, sync_channel, SyncSender, channel, Sender};
use crate::errors::{JobError, error};
use std::error::Error;

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
        };
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

trait Printable {
    fn read(&mut self) -> Result<Row, JobError>;
}

impl Printable for InputStream {
    fn read(&mut self) -> Result<Row, JobError> {
        match self.recv() {
            Ok(v) => Ok(v),
            Err(e) => Err(error(e.description())),
        }
    }
}

pub fn print(mut stream: InputStream, types: Vec<CellType>) {
    print_internal::<InputStream>(&mut stream, &types, 0);
}


struct RowReader {
    idx: usize,
    rows: Rows,
}

impl Printable for RowReader {
    fn read(&mut self) -> Result<Row, JobError> {
        if self.idx >= self.rows.rows.len() {
            return Err(error("EOF"));
        }
        self.idx += 1;
        return Ok(self.rows.rows[self.idx - 1].concrete());
    }
}


fn print_internal<T: Printable>(stream: &mut T, types: &Vec<CellType>, indent: usize) {
    let mut data: Vec<Row> = Vec::new();
    let mut has_name = false;
    let mut has_table = false;

    for val in types.iter() {
        match val.cell_type {
            CellDataType::Output(_) => has_table = true,
            CellDataType::Rows(_) => has_table = true,
            _ => (),
        }
        if val.name.len() > 0 {
            has_name = true;
        }
    }
    loop {
        match stream.read() {
            Ok(r) => {
                data.push(r)
            }
            Err(_) => break,
        }
        if data.len() == 49 || has_table {
            print_partial(data, &types, has_name, indent);
            data = Vec::new();
            data.drain(..);
        }
    }
    if !data.is_empty() {
        print_partial(data, &types, has_name, indent);
    }
}

pub fn print_partial(mut data: Vec<Row>, types: &Vec<CellType>, has_name: bool, indent: usize) {
    let mut w = vec![0; types.len()];

    if has_name {
        for (idx, val) in types.iter().enumerate() {
            w[idx] = max(w[idx], val.name.len());
        }
    }

    for r in &data {
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

    for mut r in data.into_iter() {
        let mut outputs: Vec<Output> = Vec::new();
        let mut rows: Vec<Rows> = Vec::new();
        let cell_len = r.cells.len();
        for (idx, c) in r.cells.drain(..).enumerate() {
            let cell = c.to_string();
            let spaces = if idx == cell_len - 1 { "".to_owned() } else { " ".repeat(w[idx] - cell.len()) };
            print!("{}", " ".repeat(indent * 4));
            match c.alignment() {
                Alignment::Right => print!("{}{} ", spaces, cell),
                _ => print!("{}{} ", cell, spaces),
            }

            match c {
                Cell::Output(o) => outputs.push(o),
                Cell::Rows(r) => rows.push(r),
                _ => {}
            }
        }
        println!();

        for mut o in outputs {
            print_internal(&mut o.stream, &o.types, indent + 1);
        }
        for mut r in rows {
            let t = r.types.clone();
            print_internal::<RowReader>(&mut RowReader { idx: 0, rows: r }, &t, indent + 1);
        }
    }
}
