use crate::lang::errors::{to_crush_error, CrushResult};
use crossbeam::{bounded, Receiver, Sender};
use std::cmp::min;
use std::collections::VecDeque;
use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io::{Error, Read, Write};
use std::path::PathBuf;

struct ChannelReader {
    receiver: Receiver<Box<[u8]>>,
    buff: Option<Box<[u8]>>,
}

impl Debug for ChannelReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("<channel reader>") //.map_err(|e| std::fmt::Error::default())
    }
}

impl BinaryReader for ChannelReader {
    fn clone(&self) -> Box<dyn BinaryReader + Send + Sync> {
        Box::from(ChannelReader {
            receiver: self.receiver.clone(),
            buff: None,
        })
    }
}

impl std::io::Read for ChannelReader {
    fn read(&mut self, mut dst: &mut [u8]) -> Result<usize, Error> {
        match &self.buff {
            None => match self.receiver.recv() {
                Ok(b) => {
                    if b.len() == 0 {
                        self.read(dst)
                    } else {
                        self.buff = Some(b);
                        self.read(dst)
                    }
                }

                Err(_) => Ok(0),
            },
            Some(src) => {
                if dst.len() >= src.len() {
                    let res = src.len();
                    dst.write_all(src)?;
                    self.buff = None;
                    Ok(res)
                } else {
                    let written = dst.write(src)?;
                    self.buff = Some(Box::from(&src[written..]));
                    Ok(written)
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
        let _ = self.sender.send(boxed_slice);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

pub trait BinaryReader: Read + Debug + Send + Sync {
    fn clone(&self) -> Box<dyn BinaryReader + Send + Sync>;
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

impl Read for FileReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.file.read(buf)
    }
}

impl BinaryReader for FileReader {
    fn clone(&self) -> Box<dyn BinaryReader + Send + Sync> {
        Box::from(FileReader {
            file: self.file.try_clone().unwrap(),
        })
    }
}

impl dyn BinaryReader {
    pub fn paths(mut files: Vec<PathBuf>) -> CrushResult<Box<dyn BinaryReader + Send + Sync>> {
        if files.len() == 1 {
            Ok(Box::from(FileReader::new(to_crush_error(File::open(
                files.remove(0),
            ))?)))
        } else {
            let mut readers: Vec<Box<dyn BinaryReader + Send + Sync>> = Vec::new();

            for p in files.drain(..) {
                let f = to_crush_error(File::open(p).map(|f| Box::from(FileReader::new(f))))?;
                readers.push(f)
            }
            Ok(Box::from(MultiReader {
                inner: VecDeque::from(readers),
            }))
        }
    }

    pub fn vec(bytes: &[u8]) -> Box<dyn BinaryReader + Send + Sync> {
        Box::from(VecReader {
            vec: Vec::from(bytes),
            offset: 0,
        })
    }
}

pub fn binary_channel() -> (Box<dyn Write>, Box<dyn BinaryReader + Send + Sync>) {
    let (s, r) = bounded(32);
    (
        Box::from(ChannelWriter { sender: s }),
        Box::from(ChannelReader {
            receiver: r,
            buff: None,
        }),
    )
}

struct MultiReader {
    inner: VecDeque<Box<dyn BinaryReader + Send + Sync>>,
}

impl BinaryReader for MultiReader {
    fn clone(&self) -> Box<dyn BinaryReader + Send + Sync> {
        let vec = self
            .inner
            .iter()
            .map(|r| r.as_ref().clone())
            .collect::<Vec<Box<dyn BinaryReader + Send + Sync>>>();
        Box::from(MultiReader {
            inner: VecDeque::from(vec),
        })
    }
}

impl Read for MultiReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        if self.inner.len() == 0 {
            return Ok(0);
        }
        match self.inner[0].read(buf) {
            Ok(0) => {
                self.inner.pop_front();
                self.read(buf)
            }
            Ok(s) => Ok(s),
            Err(e) => Err(e),
        }
    }
}

impl Debug for MultiReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("<multi reader>") //.map_err(|e| std::fmt::Error::default())
    }
}

struct VecReader {
    vec: Vec<u8>,
    offset: usize,
}

impl BinaryReader for VecReader {
    fn clone(&self) -> Box<dyn BinaryReader + Send + Sync> {
        Box::new(VecReader {
            vec: self.vec.clone(),
            offset: 0,
        })
    }
}

impl Read for VecReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let len = min(buf.len(), self.vec.len() - self.offset);
        buf[0..len].copy_from_slice(&self.vec[self.offset..self.offset + len]);
        self.offset += len;
        Ok(len)
    }
}

impl Debug for VecReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("<vec reader>")
    }
}
