    /**
This file implements the crush equivalent of a pipe from a regular shell.

Unlike normal pipes, these pipes can send *any* crush value. The most important
use case is to send a single value of the type TableInputStream.
 */
use crate::lang::errors::{error, send_error, to_crush_error, CrushError, CrushResult};
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::Row;
use crate::lang::value::Value;
use chrono::Duration;
use crossbeam::{bounded, unbounded, Receiver, Sender};
use lazy_static::lazy_static;

pub type RecvTimeoutError = crossbeam::channel::RecvTimeoutError;

lazy_static! {
    static ref BLACK_HOLE: ValueSender = {
        let (mut o, _) = pipe();
        o.is_pipeline = false;
        o
    };
}

pub fn black_hole() -> ValueSender {
    (*BLACK_HOLE).clone()
}

pub fn empty_channel() -> ValueReceiver {
    let (o, mut i) = pipe();
    let _ = o.send(Value::Empty());
    i.is_pipeline = false;
    i
}

#[derive(Clone)]
pub struct ValueSender {
    sender: Sender<Value>,
    is_pipeline: bool,
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
        self.send(Value::TableInputStream(input))?;
        Ok(output)
    }

    pub fn is_pipeline(&self) -> bool {
        self.is_pipeline
    }
}

#[derive(Debug, Clone)]
pub struct ValueReceiver {
    receiver: Receiver<Value>,
    is_pipeline: bool,
}

impl ValueReceiver {
    pub fn recv(&self) -> CrushResult<Value> {
        to_crush_error(self.receiver.recv())
    }

    pub fn is_pipeline(&self) -> bool {
        self.is_pipeline
    }
}

#[derive(Clone)]
pub struct OutputStream {
    sender: Sender<Row>,
    types: Vec<ColumnType>,
}

impl OutputStream {
    pub fn send(&self, row: Row) -> CrushResult<()> {
        match self.sender.send(row) {
            Ok(_) => Ok(()),
            Err(_) => send_error(),
        }
    }

    pub fn types(&self) -> &[ColumnType] {
        &self.types
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

pub fn pipe() -> (ValueSender, ValueReceiver) {
    let (send, recv) = bounded(1);
    (
        ValueSender { sender: send, is_pipeline: true },
        ValueReceiver { receiver: recv, is_pipeline: true },
    )
}

pub fn unbounded_pipe() -> (ValueSender, ValueReceiver) {
    let (send, recv) = unbounded();
    (
        ValueSender { sender: send, is_pipeline: true },
        ValueReceiver { receiver: recv, is_pipeline: true },
    )
}

pub fn pretty_printer_pipe() -> (ValueSender, ValueReceiver) {
    let (send, recv) = unbounded();
    (
        ValueSender { sender: send, is_pipeline: false },
        ValueReceiver { receiver: recv, is_pipeline: false },
    )
}

pub fn streams(signature: Vec<ColumnType>) -> (OutputStream, InputStream) {
    let (output, input) = bounded(128);
    (
        OutputStream {
            sender: output,
            types: signature.clone(),
        },
        InputStream {
            receiver: input,
            types: signature,
        },
    )
}

pub fn unlimited_streams(signature: Vec<ColumnType>) -> (OutputStream, InputStream) {
    let (output, input) = unbounded();
    (
        OutputStream {
            sender: output,
            types: signature.clone(),
        },
        InputStream {
            receiver: input,
            types: signature,
        },
    )
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
