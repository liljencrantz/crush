use crate::lang::errors::{error, send_error, to_crush_error, CrushError, CrushResult};
use crate::lang::table::ColumnType;
use crate::lang::table::Row;
use crate::lang::value::Value;
use chrono::Duration;
use crossbeam::{bounded, unbounded, Receiver, Sender};
use lazy_static::lazy_static;
use std::error::Error;

pub type RecvTimeoutError = crossbeam::channel::RecvTimeoutError;

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
    pub fn send(&self, cell: Value) -> CrushResult<()> {
        match self.sender.send(cell) {
            Ok(_) => Ok(()),
            Err(_) => send_error(),
        }
    }

    pub fn empty(&self) -> CrushResult<()> {
        self.send(Value::Empty())
    }

    pub fn initialize(&self, signature: Vec<ColumnType>) -> CrushResult<OutputStream> {
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

#[derive(Clone)]
pub struct OutputStream {
    sender: Sender<Row>,
}

impl OutputStream {
    pub fn send(&self, row: Row) -> CrushResult<()> {
        let native_output = self.sender.send(row);
        match native_output {
            Ok(_) => Ok(()),
            Err(e) => error(e.description()),
        }
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
                }
                Err(_) => return error("Index out of bounds"),
            }
        }
    }

    pub fn recv(&self) -> CrushResult<Row> {
        self.validate(to_crush_error(self.receiver.recv()))
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<Row, RecvTimeoutError> {
        self.receiver.recv_timeout(timeout.to_std().unwrap())
    }

    pub fn types(&self) -> &[ColumnType] {
        &self.types
    }

    fn validate(&self, res: CrushResult<Row>) -> CrushResult<Row> {
        match &res {
            Ok(row) => {
                if row.cells().len() != self.types.len() {
                    return error("Wrong number of columns in io");
                }
                for (c, ct) in row.cells().iter().zip(self.types.iter()) {
                    if !ct.cell_type.is(c) {
                        return error(
                            format!(
                                "Wrong cell type in io column {:?}, expected {:?}, got {:?}",
                                ct.name,
                                c.value_type(),
                                ct.cell_type
                            )
                            .as_str(),
                        );
                    }
                }
                res
            }
            Err(_) => res,
        }
    }
}

pub fn channels() -> (ValueSender, ValueReceiver) {
    let (send, recv) = bounded(1);
    (
        ValueSender { sender: send },
        ValueReceiver { receiver: recv },
    )
}

pub fn streams(signature: Vec<ColumnType>) -> (OutputStream, InputStream) {
    let (output, input) = bounded(128);
    (
        OutputStream { sender: output },
        InputStream {
            receiver: input,
            types: signature,
        },
    )
}

pub fn unlimited_streams(signature: Vec<ColumnType>) -> (OutputStream, InputStream) {
    let (output, input) = unbounded();
    (
        OutputStream { sender: output },
        InputStream {
            receiver: input,
            types: signature,
        },
    )
}

pub fn empty_channel() -> ValueReceiver {
    let (o, i) = channels();
    let _ = o.send(Value::empty_table_stream());
    i
}

pub trait CrushStream {
    fn read(&mut self) -> CrushResult<Row>;
    fn read_timeout(&mut self, timeout: Duration) -> Result<Row, RecvTimeoutError>;
    fn types(&self) -> &[ColumnType];
}

impl CrushStream for InputStream {
    fn read(&mut self) -> Result<Row, CrushError> {
        self.recv()
    }

    fn read_timeout(&mut self, timeout: Duration) -> Result<Row, RecvTimeoutError> {
        self.recv_timeout(timeout)
    }

    fn types(&self) -> &[ColumnType] {
        self.types()
    }
}

pub type Stream = Box<dyn CrushStream>;
