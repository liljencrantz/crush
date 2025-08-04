/**
This file implements the crush equivalent of a pipe from a regular shell.

Unlike normal pipes, these pipes can send *any* crush value, but they are limited to sending data
between threads inside of a single process. The most important use case is to send a single value
of the type TableInputStream.
 */
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::Row;
use crate::lang::errors::{CrushError, CrushResult, error, terminate};
use crate::lang::job_control::{ChannelBasedController, JobController, StreamControlMessage};
use crate::lang::pipe::SenderType::{BlackHole, Pipeline, Printer};
use crate::lang::value::Value;
use chrono::Duration;
use crossbeam::channel::{Receiver, Select, Sender, bounded, unbounded};
use crossbeam::select;

#[derive(Clone)]
enum SenderType {
    Printer(Sender<Value>),
    Pipeline(Sender<Value>),
    BlackHole,
}

#[derive(Clone)]
pub struct ValueSender {
    sender_type: SenderType,
}

impl ValueSender {
    pub fn send(&self, cell: Value) -> CrushResult<()> {
        match &self.sender_type {
            Printer(s) | Pipeline(s) => Ok(s.send(cell)?),
            BlackHole => Ok(()),
        }
    }

    pub fn empty(&self) -> CrushResult<()> {
        self.send(Value::Empty)
    }

    pub fn initialize(&self, signature: &[ColumnType]) -> CrushResult<TableOutputStream> {
        let (output, input) = streams(signature.to_vec());
        self.send(Value::TableInputStream(input))?;
        Ok(output)
    }

    pub fn is_pipeline(&self) -> bool {
        matches!(self.sender_type, SenderType::Pipeline(_))
    }
}

#[derive(Debug, Clone)]
pub struct ValueReceiver {
    receiver: Receiver<Value>,
    is_pipeline: bool,
}

impl ValueReceiver {
    pub fn recv(&self) -> CrushResult<Value> {
        Ok(self.receiver.recv()?)
    }

    pub fn is_pipeline(&self) -> bool {
        self.is_pipeline
    }
}

/**
A Sender that will drop any data sent to it at once.
 */
pub fn black_hole() -> ValueSender {
    ValueSender {
        sender_type: BlackHole,
    }
}

/**
A receiver that when read will return a single instance of Value::Empty
 */
pub fn empty_channel() -> ValueReceiver {
    let (o, mut i) = pipe();
    let _ = o.send(Value::Empty);
    i.is_pipeline = false;
    i
}

#[derive(Clone)]
pub struct TableOutputStream {
    sender: Sender<Row>,
    control: Option<Receiver<StreamControlMessage>>,
    types: Vec<ColumnType>,
}

impl TableOutputStream {
    pub fn send(&self, row: Row) -> CrushResult<()> {
        match &self.control {
            None => Ok(self.sender.send(row)?),
            Some(control) => {
                select! {
                    send(self.sender, row) -> res => Ok(res?),
                    recv(control) -> _ => terminate(),
                }
            }
        }
    }

    pub fn types(&self) -> &[ColumnType] {
        &self.types
    }

    pub fn interruptible(self) -> (TableOutputStream, JobController) {
        let (control_sender, control_receiver) = bounded(1);
        (
            TableOutputStream {
                sender: self.sender,
                control: Some(control_receiver),
                types: self.types,
            },
            Box::from(ChannelBasedController::new(control_sender)),
        )
    }
}

#[derive(Debug, Clone)]
pub struct TableInputStream {
    receiver: Receiver<Row>,
    types: Vec<ColumnType>,
}

impl TableInputStream {
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

    pub fn interruptible(&self) -> (Stream, JobController) {
        let (control_sender, control_receiver) = bounded(1);

        (
            Box::from(InterruptibleTableInputStream {
                input: self.clone(),
                control: control_receiver,
            }),
            Box::from(ChannelBasedController::new(control_sender)),
        )
    }

    pub fn recv(&self) -> CrushResult<Row> {
        match self.receiver.recv() {
            Ok(row) => self.validate(row),
            Err(err) => Err(err.into()),
        }
    }

    pub fn recv_timeout(&self, timeout: Duration) -> CrushResult<Row> {
        match self.receiver.recv_timeout(timeout.to_std().unwrap()) {
            Ok(row) => self.validate(row),
            Err(err) => Err(err.into()),
        }
    }

    pub fn types(&self) -> &[ColumnType] {
        &self.types
    }

    fn validate(&self, row: Row) -> CrushResult<Row> {
        if row.cells().len() != self.types.len() {
            return error(format!(
                "Pipeline expected rows to have {} columns, but received row with {} columns.",
                self.types.len(),
                row.cells().len()
            ));
        }
        for (c, ct) in row.cells().iter().zip(self.types.iter()) {
            if !ct.cell_type.is(c) {
                return error(
                    format!(
                        "Pipeline expected column `{}` to be of type `{}`, but was of type `{}`.",
                        ct.name(),
                        c.value_type(),
                        ct.cell_type
                    )
                    .as_str(),
                );
            }
        }
        Ok(row)
    }
}

/**
A Sender/Receiver pair that is bounded to only one Value on the wire before blocking.
 */
pub fn pipe() -> (ValueSender, ValueReceiver) {
    let (send, recv) = bounded(1);
    (
        ValueSender {
            sender_type: Pipeline(send),
        },
        ValueReceiver {
            receiver: recv,
            is_pipeline: true,
        },
    )
}

struct InterruptibleTableInputStream {
    input: TableInputStream,
    control: Receiver<StreamControlMessage>,
}

impl CrushStream for InterruptibleTableInputStream {
    fn read(&mut self) -> CrushResult<Row> {
        select! {
            recv(self.input.receiver) -> r => Ok(r?),
            recv(self.control) -> msg => {
                match msg {
                    Ok(StreamControlMessage::Terminate) => {terminate()}
                    Ok(StreamControlMessage::Pause) => {
                        println!("PAUSE!!!");
                        self.control.recv()?;
                        self.read()
                    }
                    Ok(StreamControlMessage::Resume) => {panic!()}
                    Err(e) => {
                        Err(e.into())
                    }
                }
            }
        }
    }

    fn read_timeout(&mut self, timeout: Duration) -> CrushResult<Row> {
        let mut sel = Select::new();
        let oper1 = sel.recv(&self.input.receiver);
        let oper2 = sel.recv(&self.control);

        let oper = sel.select_timeout(timeout.to_std()?);
        match oper {
            Err(e) => Err(e.into()),
            Ok(oper) => match oper.index() {
                i if i == oper1 => Ok(oper.recv(&self.input.receiver)?),
                i if i == oper2 => terminate(),
                _ => unreachable!(),
            },
        }
    }

    fn types(&self) -> &[ColumnType] {
        self.input.types()
    }
}

/**
A Sender/Receiver pair that is bounded to only one Value on the wire before blocking.
 */
pub fn printer_pipe() -> (ValueSender, ValueReceiver) {
    let (send, recv) = bounded(1);
    (
        ValueSender {
            sender_type: Printer(send),
        },
        ValueReceiver {
            receiver: recv,
            is_pipeline: false,
        },
    )
}

pub fn streams(signature: Vec<ColumnType>) -> (TableOutputStream, TableInputStream) {
    let (output, input) = bounded(128);
    (
        TableOutputStream {
            sender: output,
            types: signature.clone(),
            control: None,
        },
        TableInputStream {
            receiver: input,
            types: signature,
        },
    )
}

pub fn unlimited_streams(signature: Vec<ColumnType>) -> (TableOutputStream, TableInputStream) {
    let (output, input) = unbounded();
    (
        TableOutputStream {
            sender: output,
            types: signature.clone(),
            control: None,
        },
        TableInputStream {
            receiver: input,
            types: signature,
        },
    )
}

pub trait CrushStream {
    fn read(&mut self) -> CrushResult<Row>;
    fn read_timeout(&mut self, timeout: Duration) -> CrushResult<Row>;
    fn types(&self) -> &[ColumnType];
}

impl CrushStream for TableInputStream {
    fn read(&mut self) -> Result<Row, CrushError> {
        self.recv()
    }

    fn read_timeout(&mut self, timeout: Duration) -> CrushResult<Row> {
        self.recv_timeout(timeout)
    }

    fn types(&self) -> &[ColumnType] {
        self.types()
    }
}

pub type Stream = Box<dyn CrushStream>;
