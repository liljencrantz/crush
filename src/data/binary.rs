use crate::errors::{JobResult, to_job_error};
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::{Error, Read, Write, ErrorKind};
use std::sync::mpsc::{SyncSender, Receiver, sync_channel, RecvError};
use std::fmt::{Debug, Formatter};
use std::fs::File;
use serde_json::to_vec;
use std::path::Path;


pub struct ChannelReader {
    receiver: Receiver<Box<[u8]>>,
    buff: Option<Box<[u8]>>,
}

impl Debug for ChannelReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("<channel reader>")//.map_err(|e| std::fmt::Error::default())
    }
}

impl std::io::Read for ChannelReader {
    fn read(&mut self, mut dst: &mut [u8]) -> Result<usize, Error> {
        match &self.buff {
            None => {
                match self.receiver.recv() {
                    Ok(b) => {
                        if b.len() == 0 {
                            Ok(0)
                        } else {
                            self.buff = Some(b);
                            self.read(dst)
                        }
                    }

                    Err(e) => {
                        Ok(0)
                    }
                }
            }
            Some(src) => {
                if dst.len() >= src.len() {
                    let res = src.len();
                    dst.write(src);
                    self.buff = None;
                    Ok(res)
                } else {
                    dst.write(src);
                    self.buff = Some(Box::from(&src[dst.len()..]));
                    Ok(dst.len())
                }
            }
        }
    }
}

pub struct ChannelWriter {
    sender: SyncSender<Box<[u8]>>,
}

impl std::io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        let boxed_slice: Box<[u8]> = buf.into();
        self.sender.send(boxed_slice);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

pub fn binary() -> JobResult<(ChannelWriter, ChannelReader)> {
    let (s, r) = sync_channel(32);
    Ok((ChannelWriter { sender: s }, ChannelReader { receiver: r, buff: None }))
}

pub struct BinaryReader {
    pub reader: Box<dyn Read + Send>,
}

impl BinaryReader {
    pub fn new<T: 'static + Read + Send>(f: T) -> BinaryReader {
        BinaryReader {
            reader: Box::new(f)
        }
    }

    pub fn from(file: &Path) -> JobResult<BinaryReader>{
        return Ok(BinaryReader::new(to_job_error(File::open(file))?))
    }
}

impl Debug for BinaryReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("<binary stream>")//.map_err(|e| std::fmt::Error::default())
    }
}
