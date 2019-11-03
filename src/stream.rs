use crate::data::{ColumnType, Cell};
use crate::data::{Row, Rows};
use std::sync::mpsc::{Receiver, sync_channel, SyncSender, channel, Sender};
use crate::errors::{JobError, error, JobResult, to_job_error};
use std::error::Error;
use crate::replace::Replace;

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
        self.validate(to_job_error(self.receiver.recv()))
    }

    pub fn get_type(&self) -> &Vec<ColumnType> {
        &self.input_type
    }

    fn validate(&self, res: JobResult<Row>) -> JobResult<Row> {
        match &res {
            Ok(row) => {
                if row.cells.len() != self.input_type.len() {
                    return Err(error("Wrong number of columns in input"));
                }
                for (c, ct) in row.cells.iter().zip(self.input_type.iter()) {
                    if c.cell_type() != ct.cell_type {
                        return Err(error(format!(
                            "Wrong cell type in input column {:?}, expected {:?}, got {:?}",
                            ct.name,
                            c.cell_type(),
                            ct.cell_type).as_str()));
                    }
                }
                res
            },
            Err(_) => res,
        }
    }
}

pub fn streams() -> (UninitializedOutputStream, UninitializedInputStream) {
    let (type_send, type_recv) = sync_channel(1);
    let (send, recv) = sync_channel(128);
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


pub struct RowsReader {
    idx: usize,
    rows: Rows,
    row_type: Vec<ColumnType>,
}

impl RowsReader {
    pub fn new(rows: Rows) -> RowsReader {
        RowsReader{
            idx: 0,
            row_type: rows.types.clone(),
            rows,
        }
    }
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
