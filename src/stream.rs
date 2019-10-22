use crate::data::{CellType, CellFnurp, Cell, JobOutput};
use crate::data::{Alignment, Row, Rows};
use std::cmp::max;
use std::sync::mpsc::{Receiver, sync_channel, SyncSender, channel, Sender};
use crate::errors::{JobError, error};
use std::error::Error;
use crate::printer::Printer;
use crate::replace::Replace;
use std::thread;

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

pub trait Readable {
    fn read(&mut self) -> Result<Row, JobError>;
}

impl Readable for InputStream {
    fn read(&mut self) -> Result<Row, JobError> {
        match self.recv() {
            Ok(v) => Ok(v),
            Err(e) => Err(error(e.description())),
        }
    }
}

pub fn spawn_print_thread(printer: &Printer, output: JobOutput) {
    let p = printer.clone();
    thread::Builder::new()
        .name("output_formater".to_string())
        .spawn(move || print(&p, output.stream, output.types)
        );
}

pub fn print(printer: &Printer, mut stream: InputStream, types: Vec<CellFnurp>) {
    print_internal::<InputStream>(printer, &mut stream, &types, 0);
}

pub struct RowsReader {
    idx: usize,
    rows: Rows,
}

impl Readable for RowsReader {
    fn read(&mut self) -> Result<Row, JobError> {
        if self.idx >= self.rows.rows.len() {
            return Err(error("EOF"));
        }
        self.idx += 1;
        return Ok(self.rows.rows.replace(self.idx - 1, Row{cells:vec![Cell::Integer(0)],}));
    }
}

fn print_internal<T: Readable>(printer: &Printer, stream: &mut T, types: &Vec<CellFnurp>, indent: usize) {
    let mut data: Vec<Row> = Vec::new();
    let mut has_name = false;
    let mut has_table = false;

    for val in types.iter() {
        match val.cell_type {
            CellType::Output(_) => has_table = true,
            CellType::Rows(_) => has_table = true,
            _ => (),
        }
        if val.name.is_some() {
            has_name = true;
        }
    }
    loop {
        match stream.read() {
            Ok(r) => {
                data.push(r.concrete())
            }
            Err(_) => break,
        }
        if data.len() == 49 || has_table {
            print_partial(printer, data, &types, has_name, indent);
            data = Vec::new();
            data.drain(..);
        }
    }
    if !data.is_empty() {
        print_partial(printer, data, &types, has_name, indent);
    }
}

fn calculate_header_width(w: &mut Vec<usize>, types: &Vec<CellFnurp>, has_name: bool) {
    if has_name {
        for (idx, val) in types.iter().enumerate() {
            w[idx] = max(w[idx], val.len_or_0());
        }
    }
}

fn calculate_body_width(w: &mut Vec<usize>,  data: &Vec<Row>, col_count: usize) {
    for r in data {
        assert_eq!(col_count, r.cells.len());
        for (idx, c) in r.cells.iter().enumerate() {
            let l = c.to_string().len();
            w[idx] = max(w[idx], l);
        }
    }
}

fn print_header(printer: &Printer, w: &Vec<usize>, types: &Vec<CellFnurp>, has_name: bool, indent: usize) {
    if has_name {
        let mut header = " ".repeat(indent * 4);
        for (idx, val) in types.iter().enumerate() {
            header += val.val_or_empty();
            header += &" ".repeat(w[idx] - val.len_or_0() + 1);
        }
        printer.line(header.as_str())
    }
}

fn print_row(printer: &Printer, w: &Vec<usize>, mut r: Row, indent: usize, rows: &mut Vec<Rows>) {
    let cell_len = r.cells.len();
    let mut row = " ".repeat(indent * 4);
    for (idx, c) in r.cells.drain(..).enumerate() {
        let cell = c.to_string();
        let spaces = if idx == cell_len - 1 { "".to_string() } else { " ".repeat(w[idx] - cell.len()) };
        match c.alignment() {
            Alignment::Right => {row += spaces.as_str(); row += cell.as_str(); row += " "},
            _ => {row += cell.as_str(); row += spaces.as_str(); row += " "},
        }

        match c {
            Cell::Rows(r) => rows.push(r),
            _ => {}
        }
    }
    printer.line(row.as_str());
}

fn print_body(printer: &Printer, w: &Vec<usize>,  data: Vec<Row>, indent: usize) {
    for r in data.into_iter() {
        let mut rows = Vec::new();
        print_row(printer, w, r, indent, &mut rows);
        for r in rows {
            let t = r.types.clone();
            print_internal::<RowsReader>(printer, &mut RowsReader { idx: 0, rows: r }, &t, indent + 1);
        }
    }
}

fn print_partial(printer: &Printer, data: Vec<Row>, types: &Vec<CellFnurp>, has_name: bool, indent: usize) {
    let mut w = vec![0; types.len()];

    calculate_header_width(&mut w, types, has_name);
    calculate_body_width(&mut w, &data, types.len());

    print_header(printer, &w, types, has_name, indent);
    print_body(printer, &w, data, indent)
}
