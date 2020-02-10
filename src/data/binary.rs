use crate::errors::{CrushResult, to_job_error};
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::{Error, Read, Write, ErrorKind};
use crossbeam::{Receiver, bounded, Sender};
use std::fmt::{Debug, Formatter};
use std::fs::File;
use serde_json::to_vec;
use std::path::Path;


struct ChannelReader {
    receiver: Receiver<Box<[u8]>>,
    buff: Option<Box<[u8]>>,
}

impl Debug for ChannelReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("<channel reader>")//.map_err(|e| std::fmt::Error::default())
    }
}

impl BinaryReader for ChannelReader {
    fn reader(&self) -> Box<dyn Read> {
        Box::from(ChannelReader { receiver: self.receiver.clone(), buff: None })
    }

    fn clone(&self) -> Box<dyn BinaryReader> {
        Box::from(ChannelReader { receiver: self.receiver.clone(), buff: None })
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

struct ChannelWriter {
    sender: Sender<Box<[u8]>>,
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

pub trait BinaryReader: Debug + Send {
    fn reader(&self) -> Box<dyn Read>;
    fn clone(&self) -> Box<dyn BinaryReader>;
}

struct FileReader {
    file: File,
}

impl FileReader {
    pub fn new(file: File) -> FileReader {
        FileReader { file }
    }

}

impl Debug for FileReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("<file reader>")
    }
}

impl BinaryReader for FileReader {
    fn reader(&self) -> Box<dyn Read> {
        Box::from(self.file.try_clone().unwrap())
    }

    fn clone(&self) -> Box<dyn BinaryReader> {
        Box::from(FileReader { file: self.file.try_clone().unwrap() })
    }
}

impl dyn BinaryReader {
    pub fn from(file: &Path) -> CrushResult<Box<dyn BinaryReader>> {
        return Ok(Box::from(FileReader::new(to_job_error(File::open(file))?)));
    }
}

pub fn binary_channel() -> CrushResult<(Box<dyn Write>, Box<dyn BinaryReader>)> {
    let (s, r) = bounded(32);
    Ok((
        Box::from(ChannelWriter { sender: s }),
        Box::from(ChannelReader { receiver: r, buff: None })
    ))
}
