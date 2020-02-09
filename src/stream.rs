use crate::data::{ColumnType, Value};
use crate::data::{Row, Rows, Stream};
use crossbeam::{Receiver, bounded, unbounded, Sender};
use crate::errors::{JobError, error, JobResult, to_job_error};
use crate::replace::Replace;

pub struct ValueSender {
    sender: Sender<Value>,
}

impl ValueSender {
    pub fn send(self, cell: Value) -> JobResult<()> {
        to_job_error(self.sender.send(cell))?;
        Ok(())
    }

    pub fn initialize(self, signature: Vec<ColumnType>) -> JobResult<OutputStream> {
        let (output, input) = streams(signature);
        self.send(Value::Stream(Stream { stream: input }))?;
        Ok(output)
    }
}

#[derive(Debug)]
pub struct ValueReceiver {
    receiver: Receiver<Value>,
}

impl ValueReceiver {
    pub fn recv(self) -> JobResult<Value> {
        to_job_error(self.receiver.recv())
    }
}

pub enum OutputStream {
    Sync(Sender<Row>),
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

#[derive(Debug, Clone)]
pub struct InputStream {
    receiver: Receiver<Row>,
    input_type: Vec<ColumnType>,
}

impl InputStream {
    pub fn recv(&self) -> JobResult<Row> {
        self.validate(to_job_error(self.receiver.recv()))
    }

    pub fn get_type(&self) -> &Vec<ColumnType> {
        &self.input_type
    }

    fn validate(&self, res: JobResult<Row>) -> JobResult<Row> {
        match &res {
            Ok(row) => {
                if row.cells().len() != self.input_type.len() {
                    return Err(error("Wrong number of columns in input"));
                }
                for (c, ct) in row.cells().iter().zip(self.input_type.iter()) {
                    if c.value_type() != ct.cell_type {
                        return Err(error(format!(
                            "Wrong cell type in input column {:?}, expected {:?}, got {:?}",
                            ct.name,
                            c.value_type(),
                            ct.cell_type).as_str()));
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
    (OutputStream::Sync(output), InputStream { receiver: input, input_type: signature })
}

pub fn unlimited_streams(signature: Vec<ColumnType>) -> (OutputStream, InputStream) {
    let (output, input) = unbounded();
    (OutputStream::Async(output), InputStream { receiver: input, input_type: signature })
}

pub fn empty_channel() -> ValueReceiver {
    let (o, i) = channels();
    o.send(Value::empty_stream());
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
