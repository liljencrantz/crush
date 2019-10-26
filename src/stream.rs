use crate::data::{CellType, ColumnType, Cell, JobOutput};
use crate::data::{Alignment, Row, Rows};
use std::cmp::max;
use std::sync::mpsc::{Receiver, sync_channel, SyncSender, channel, Sender, RecvError};
use crate::errors::{JobError, error, JobResult, to_job_error};
use std::error::Error;
use crate::printer::Printer;
use crate::replace::Replace;
use std::thread;

pub enum UninitializedOutputStream {
    Sync(SyncSender<Vec<ColumnType>>, SyncSender<Row>),
    Async(SyncSender<Vec<ColumnType>>, Sender<Row>),
}

impl UninitializedOutputStream {
    pub fn initialize(self, output_type: Vec<ColumnType>) -> JobResult<OutputStream> {
        match self {
            UninitializedOutputStream::Sync(s, s2) => {
                to_job_error(s.send(output_type.clone()))?;
                Ok(OutputStream::Sync(s2))
            }
            UninitializedOutputStream::Async(s, s2) => {
                to_job_error(s.send(output_type.clone()))?;
                Ok(OutputStream::Async(s2))
            }
        }
    }
}

#[derive(Debug)]
pub struct UninitializedInputStream {
    type_input: Receiver<Vec<ColumnType>>,
    input: Receiver<Row>,
}

impl UninitializedInputStream {
    pub fn initialize(self) -> JobResult<InputStream> {
        match self.type_input.recv() {
            Ok(t) => Ok(InputStream { receiver: self.input, input_type: t }),
            Err(e) => Err(error(e.description())),
        }
    }
}

pub enum OutputStream {
    Sync(SyncSender<Row>),
    Async(Sender<Row>),
}


impl OutputStream {
    pub fn send(&self, row: Row) -> JobResult<()> {
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

#[derive(Debug)]
pub struct InputStream {
    receiver: Receiver<Row>,
    input_type: Vec<ColumnType>,
}


impl InputStream {
    pub fn recv(&self) -> JobResult<Row> {
        to_job_error(self.receiver.recv())
    }

    pub fn get_type(&self) -> &Vec<ColumnType> {
        &self.input_type
    }
}

pub fn streams() -> (UninitializedOutputStream, UninitializedInputStream) {
    let (type_send, type_recv) = sync_channel(1);
    let (send, recv) = sync_channel(200000);
    (UninitializedOutputStream::Sync(type_send, send), UninitializedInputStream { type_input: type_recv, input: recv })
}

pub fn unlimited_streams() -> (UninitializedOutputStream, UninitializedInputStream) {
    let (type_send, type_recv) = sync_channel(1);
    let (send, recv) = channel();
    (UninitializedOutputStream::Async(type_send, send), UninitializedInputStream { type_input: type_recv, input: recv })
}

pub fn empty_stream() -> UninitializedInputStream {
    let (o, i) = unlimited_streams();
    o.initialize(vec![]);
    i
}

pub trait Readable {
    fn read(&mut self) -> JobResult<Row>;
    fn get_type(&self) -> &Vec<ColumnType>;
}

impl Readable for InputStream {
    fn read(&mut self) -> Result<Row, JobError> {
        match self.recv() {
            Ok(v) => Ok(v),
            Err(e) => Err(error(&e.message)),
        }
    }

    fn get_type(&self) -> &Vec<ColumnType> {
        self.get_type()
    }
}

pub fn spawn_print_thread(printer: &Printer, output: UninitializedInputStream) {
    let p = printer.clone();
    thread::Builder::new()
        .name("output_formater".to_string())
        .spawn(move || {
            match output.initialize() {
                Ok(out) => print(&p, out),
                Err(e) => p.job_error(e),
            }
        }
        );
}

pub fn print(printer: &Printer, mut stream: InputStream) {
    print_internal(printer, &mut stream, 0);
}

pub struct RowsReader {
    idx: usize,
    rows: Rows,
    row_type: Vec<ColumnType>,
}

impl Readable for RowsReader {
    fn read(&mut self) -> Result<Row, JobError> {
        if self.idx >= self.rows.rows.len() {
            return Err(error("EOF"));
        }
        self.idx += 1;
        return Ok(self.rows.rows.replace(self.idx - 1, Row { cells: vec![Cell::Integer(0)] }));
    }

    fn get_type(&self) -> &Vec<ColumnType> {
        &self.row_type
    }
}

fn print_internal(printer: &Printer, stream: &mut impl Readable, indent: usize) {
    let mut data: Vec<Row> = Vec::new();
    let mut has_name = false;
    let mut has_table = false;

    for val in stream.get_type().iter() {
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
            print_partial(printer, data, stream.get_type(), has_name, indent);
            data = Vec::new();
            data.drain(..);
        }
    }
    if !data.is_empty() {
        print_partial(printer, data, stream.get_type(), has_name, indent);
    }
}

fn calculate_header_width(w: &mut Vec<usize>, types: &Vec<ColumnType>, has_name: bool) {
    if has_name {
        for (idx, val) in types.iter().enumerate() {
            w[idx] = max(w[idx], val.len_or_0());
        }
    }
}

fn calculate_body_width(w: &mut Vec<usize>, data: &Vec<Row>, col_count: usize) {
    for r in data {
        assert_eq!(col_count, r.cells.len());
        for (idx, c) in r.cells.iter().enumerate() {
            let l = c.to_string().len();
            w[idx] = max(w[idx], l);
        }
    }
}

fn print_header(printer: &Printer, w: &Vec<usize>, types: &Vec<ColumnType>, has_name: bool, indent: usize) {
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
            Alignment::Right => {
                row += spaces.as_str();
                row += cell.as_str();
                row += " "
            }
            _ => {
                row += cell.as_str();
                row += spaces.as_str();
                row += " "
            }
        }

        match c {
            Cell::Rows(r) => rows.push(r),
            _ => {}
        }
    }
    printer.line(row.as_str());
}

fn print_body(printer: &Printer, w: &Vec<usize>, data: Vec<Row>, indent: usize) {
    for r in data.into_iter() {
        let mut rows = Vec::new();
        print_row(printer, w, r, indent, &mut rows);
        for r in rows {
            print_internal(printer, &mut RowsReader { idx: 0, row_type: r.types.clone(), rows: r }, indent + 1);
        }
    }
}

fn print_partial(printer: &Printer, data: Vec<Row>, types: &Vec<ColumnType>, has_name: bool, indent: usize) {
    let mut w = vec![0; types.len()];

    calculate_header_width(&mut w, types, has_name);
    calculate_body_width(&mut w, &data, types.len());

    print_header(printer, &w, types, has_name, indent);
    print_body(printer, &w, data, indent)
}
