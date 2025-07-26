use crate::lang::data::binary::binary_channel;
use crate::lang::errors::{CrushError, CrushResult, argument_error, command_error, data_error};
use crate::lang::pipe::ValueSender;
use crate::lang::value::{Value, ValueType};
use crate::util::file::cwd;
use crate::util::glob::Glob;
use crate::util::regex::RegexFileMatcher;
use regex::Regex;
use std::convert::TryFrom;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/**
A type representing one or more files.
 */
#[derive(Debug, Clone)]
pub enum Files {
    File(Arc<Path>),
    Glob(Glob),
    Regex(Regex),
}

impl TryFrom<Value> for Files {
    type Error = CrushError;

    fn try_from(value: Value) -> CrushResult<Self> {
        match value {
            Value::File(v) => Ok(Files::File(v)),
            Value::Glob(v) => Ok(Files::Glob(v)),
            Value::Regex(_, v) => Ok(Files::Regex(v)),
            v => command_error(format!(
                "Invalid type `{}`, expected `one_of $file $glob $binary`.",
                v.value_type()
            )),
        }
    }
}

impl TryInto<Vec<PathBuf>> for Files {
    type Error = CrushError;

    fn try_into(self) -> CrushResult<Vec<PathBuf>> {
        match self {
            Files::File(p) => Ok(vec![p.to_path_buf()]),
            Files::Glob(pattern) => {
                let mut tmp = Vec::new();
                pattern.glob_files(&cwd()?, &mut tmp)?;
                Ok(tmp.into_iter().collect())
            }
            Files::Regex(pattern) => {
                let mut tmp = Vec::new();
                pattern.match_files(&cwd()?, &mut tmp)?;
                Ok(tmp.into_iter().collect())
            }
        }
    }
}

pub fn into_paths(files: Vec<Files>) -> CrushResult<Vec<PathBuf>> {
    let mut res = Vec::new();
    for i in files {
        res.append(&mut <Files as TryInto<Vec<PathBuf>>>::try_into(i)?);
    }
    Ok(res)
}

impl TryInto<Box<dyn Write>> for Files {
    type Error = CrushError;

    fn try_into(self) -> Result<Box<dyn Write>, Self::Error> {
        let vec: Vec<_> = self.try_into()?;
        match vec.len() {
            1 => Ok(Box::from(File::create(&vec[0])?)),
            n => command_error("Invalid output file"),
        }
    }
}

pub fn writer(files: Option<Files>, output: ValueSender) -> CrushResult<Box<dyn Write>> {
    match files {
        None => {
            let (w, r) = binary_channel();
            output.send(Value::BinaryInputStream(r))?;
            Ok(w)
        }
        Some(file) => {
            output.send(Value::Empty)?;
            Ok(file.try_into()?)
        }
    }
}

pub fn path(files: Option<Files>, fallback: impl Into<PathBuf>) -> CrushResult<PathBuf> {
    match files {
        None => Ok(fallback.into()),
        Some(file) => {
            let mut dir: Vec<PathBuf> = file.try_into()?;
            match dir.len() {
                1 => Ok(dir.pop().unwrap()),
                n => return command_error("Invalid directory."),
            }
        }
    }
}
