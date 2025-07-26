use crate::lang::data::binary::BinaryReader;
use crate::lang::errors::{CrushError, CrushResult, command_error};
use crate::lang::pipe::ValueReceiver;
use crate::lang::value::{BinaryInputStream, Value};
use crate::util::file::cwd;
use crate::util::glob::Glob;
use crate::util::regex::RegexFileMatcher;
use regex::Regex;
use std::collections::VecDeque;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

/// A type representing a value with a binary representation. It is used in the signature of builtin commands that
/// accept any type of binary value as arguments.
pub enum BinaryInput {
    BinaryInputStream(BinaryInputStream),
    Binary(Arc<[u8]>),
    /// Will be implicitly converted to bytes using utf-8 encoding.
    String(Arc<str>),
    File(Arc<Path>),
    Glob(Glob),
    Regex(Regex),
}

impl TryFrom<Value> for BinaryInput {
    type Error = CrushError;

    fn try_from(value: Value) -> CrushResult<Self> {
        match value {
            Value::Binary(v) => Ok(BinaryInput::Binary(v)),
            Value::BinaryInputStream(v) => Ok(BinaryInput::BinaryInputStream(v)),
            Value::String(v) => Ok(BinaryInput::String(v)),
            Value::File(v) => Ok(BinaryInput::File(v)),
            Value::Glob(v) => Ok(BinaryInput::Glob(v)),
            Value::Regex(_, v) => Ok(BinaryInput::Regex(v)),
            v => command_error(format!(
                "Invalid type `{}`, expected `one_of $file $string $binary $binary_input_string $glob $regex`.",
                v.value_type()
            )),
        }
    }
}

pub trait ToReader {
    fn to_reader(self, fallback: ValueReceiver)
    -> CrushResult<Box<dyn BinaryReader + Send + Sync>>;
}

impl ToReader for Vec<BinaryInput> {
    fn to_reader(
        mut self,
        fallback: ValueReceiver,
    ) -> CrushResult<Box<dyn BinaryReader + Send + Sync>> {
        if self.is_empty() {
            match fallback.recv()? {
                Value::BinaryInputStream(b) => Ok(b),
                Value::Binary(b) => Ok(<dyn BinaryReader>::vec(&b)),
                Value::String(s) => Ok(<dyn BinaryReader>::vec(s.as_bytes())),
                v => command_error(format!(
                    "Expected `one_of $binary $binary_input_stream $string`, but got `{}`",
                    v.value_type()
                )),
            }
        } else {
            let mut readers: Vec<Box<dyn BinaryReader + Send + Sync>> = Vec::new();
            for i in self.drain(..) {
                match i {
                    BinaryInput::File(p) => readers.push(Box::from(
                        crate::lang::data::binary::FileReader::new(File::open(p)?),
                    )),
                    BinaryInput::BinaryInputStream(s) => readers.push(Box::from(s)),
                    BinaryInput::Binary(b) => readers.push(<dyn BinaryReader>::vec(&b)),
                    BinaryInput::String(s) => readers.push(<dyn BinaryReader>::vec(&s.as_bytes())),
                    BinaryInput::Glob(g) => {
                        let mut paths = Vec::new();
                        g.glob_files(&cwd()?, &mut paths)?;
                        for path in paths {
                            readers.push(Box::from(crate::lang::data::binary::FileReader::new(
                                File::open(path)?,
                            )));
                        }
                    }
                    BinaryInput::Regex(re) => {
                        let mut paths = Vec::new();
                        re.match_files(&cwd()?, &mut paths)?;
                        for path in paths {
                            readers.push(Box::from(crate::lang::data::binary::FileReader::new(
                                File::open(path)?,
                            )));
                        }
                    }
                }
            }
            Ok(Box::from(crate::lang::data::binary::MultiReader::new(
                VecDeque::from(readers),
            )))
        }
    }
}

pub trait ToPaths {
    fn to_paths(self) -> CrushResult<Vec<Arc<Path>>>;
}

pub fn input_reader(input: BinaryInput) -> CrushResult<Box<dyn BinaryReader + Send + Sync>> {
    let mut readers: Vec<Box<dyn BinaryReader + Send + Sync>> = Vec::new();
    match input {
        BinaryInput::File(p) => readers.push(Box::from(
            crate::lang::data::binary::FileReader::new(File::open(p)?),
        )),
        BinaryInput::BinaryInputStream(s) => readers.push(Box::from(s)),
        BinaryInput::Binary(b) => readers.push(<dyn BinaryReader>::vec(&b)),
        BinaryInput::String(s) => readers.push(<dyn BinaryReader>::vec(&s.as_bytes())),
        BinaryInput::Glob(g) => {
            let mut paths = Vec::new();
            g.glob_files(&cwd()?, &mut paths)?;
            for path in paths {
                readers.push(Box::from(crate::lang::data::binary::FileReader::new(
                    File::open(path)?,
                )));
            }
        }
        BinaryInput::Regex(re) => {
            let mut paths = Vec::new();
            re.match_files(&cwd()?, &mut paths)?;
            for path in paths {
                readers.push(Box::from(crate::lang::data::binary::FileReader::new(
                    File::open(path)?,
                )));
            }
        }
    }
    Ok(Box::from(crate::lang::data::binary::MultiReader::new(
        VecDeque::from(readers),
    )))
}
