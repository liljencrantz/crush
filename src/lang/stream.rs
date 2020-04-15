use crate::lang::table::ColumnType;
use crate::lang::value::Value;
use crate::lang::{table::Row};
use crossbeam::{Receiver, bounded, unbounded, Sender};
use crate::lang::errors::{CrushError, error, CrushResult, to_crush_error, send_error};
use lazy_static::lazy_static;

lazy_static! {
    static ref BLACK_HOLE: ValueSender = {
        let (o, _) = channels();
        o
    };
}

pub fn black_hole() -> ValueSender {
    (*BLACK_HOLE).clone()
}

#[derive(Clone)]
pub struct ValueSender {
    sender: Sender<Value>,
}

impl ValueSender {
    pub fn send(self, cell: Value) -> CrushResult<()> {
        match self.sender.send(cell) {
            Ok(_) => Ok(()),
            Err(_) => send_error(),
        }
    }

    pub fn initialize(self, signature: Vec<ColumnType>) -> CrushResult<OutputStream> {
        let (output, input) = streams(signature);
        self.send(Value::TableStream(input))?;
        Ok(output)
    }
}

#[derive(Debug, Clone)]
pub struct ValueReceiver {
    receiver: Receiver<Value>,
}

impl ValueReceiver {
    pub fn recv(&self) -> CrushResult<Value> {
        to_crush_error(self.receiver.recv())
    }
}

pub enum OutputStream {
    Sync(Sender<Row>),
    Async(Sender<Row>),
}

impl OutputStream {
    pub fn send(&self, row: Row) -> CrushResult<()> {
        let native_output = match self {
            OutputStream::Sync(s) => s.send(row),
            OutputStream::Async(s) => s.send(row),
        };
        return match native_output {
            Ok(_) => Ok(()),
            Err(_) => error("Broken pipe"),
        };
    }
}

#[derive(Debug, Clone)]
pub struct InputStream {
    receiver: Receiver<Row>,
    types: Vec<ColumnType>,
}

impl InputStream {

    pub fn get(&self, idx: i128) -> CrushResult<Row> {
        let mut i = 0i128;
        loop {
            match self.recv() {
                Ok(row) => {
                    if i == idx {
                        return Ok(row);
                    }
                    i += 1;
                },
                Err(_) => return error("Index out of bounds"),
            }
        }
    }

    pub fn recv(&self) -> CrushResult<Row> {
        self.validate(to_crush_error(self.receiver.recv()))
    }

    pub fn types(&self) -> &[ColumnType] {
        &self.types
    }

    fn validate(&self, res: CrushResult<Row>) -> CrushResult<Row> {
        match &res {
            Ok(row) => {
                if row.cells().len() != self.types.len() {
                    return error("Wrong number of columns in input");
                }
                for (c, ct) in row.cells().iter().zip(self.types.iter()) {
                    if c.value_type() != ct.cell_type {
                        return error(format!(
                            "Wrong cell type in input column {:?}, expected {:?}, got {:?}",
                            ct.name,
                            c.value_type(),
                            ct.cell_type).as_str());
                    }
                }
                res
            },
            Err(_) => res,
        }
    }
}

pub fn channels() -> (ValueSender, ValueReceiver) {
    let (send, recv) = bounded(1);
    (ValueSender {sender: send}, ValueReceiver { receiver: recv })
}

pub fn streams(signature: Vec<ColumnType>) -> (OutputStream, InputStream) {
    let (output, input) = bounded(128);
    (OutputStream::Sync(output), InputStream { receiver: input, types: signature })
}

pub fn unlimited_streams(signature: Vec<ColumnType>) -> (OutputStream, InputStream) {
    let (output, input) = unbounded();
    (OutputStream::Async(output), InputStream { receiver: input, types: signature })
}

pub fn empty_channel() -> ValueReceiver {
    let (o, i) = channels();
    let _ = o.send(Value::empty_table_stream());
    i
}

pub trait Readable {
    fn read(&mut self) -> CrushResult<Row>;
    fn types(&self) -> &[ColumnType];
}

impl Readable for InputStream {
    fn read(&mut self) -> Result<Row, CrushError> {
        match self.recv() {
            Ok(v) => Ok(v),
            Err(e) => error(&e.message),
        }
    }

    fn types(&self) -> &[ColumnType] {
        self.types()
    }
}
