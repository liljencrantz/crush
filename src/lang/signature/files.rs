use crate::lang::data::binary::{BinaryWriter, binary_channel};
use crate::lang::errors::{CrushError, CrushResult, command_error};
use crate::lang::job_control::ChannelBasedController;
use crate::lang::pipe::ValueSender;
use crate::lang::state::handles::CommandHandle;
use crate::lang::value::Value;
use crate::util::file::cwd;
use crate::util::glob::Glob;
use crate::util::regex::RegexFileMatcher;
use crossbeam::channel::unbounded;
use regex::Regex;
use std::convert::TryFrom;
use std::fs::File;
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
    String(Arc<str>),
}

impl TryFrom<Value> for Files {
    type Error = CrushError;

    fn try_from(value: Value) -> CrushResult<Self> {
        match value {
            Value::String(v) => Ok(Files::String(v)),
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
            Files::String(p) => Ok(vec![PathBuf::from(p.as_ref())]),
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

impl TryInto<Box<dyn BinaryWriter>> for Files {
    type Error = CrushError;

    fn try_into(self) -> Result<Box<dyn BinaryWriter>, Self::Error> {
        let vec: Vec<_> = self.try_into()?;
        match vec.len() {
            0 => command_error("No write target specified."),
            1 => Ok(Box::from(File::create(&vec[0])?)),
            n => command_error(format!(
                "Single write targets expected, found {} different write targets.",
                n
            )),
        }
    }
}

/// Same job-control wiring as `binary_input::register_control`, for the write side --
/// see that function's doc comment. Only a channel-backed writer does anything with it;
/// a plain file has no reader to wait on and just ignores this via `BinaryWriter`'s
/// default no-op.
fn register_control(writer: &mut Box<dyn BinaryWriter>, command_handle: &CommandHandle) {
    let (control_sender, control_receiver) = unbounded();
    command_handle.register(Box::from(ChannelBasedController::new(control_sender)));
    writer.register_control(control_receiver);
}

pub fn writer(
    files: Option<Files>,
    output: ValueSender,
    command_handle: &CommandHandle,
) -> CrushResult<Box<dyn BinaryWriter>> {
    match files {
        None => {
            let (mut w, r) = binary_channel();
            register_control(&mut w, command_handle);
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
                0 => command_error("No path specified."),
                1 => Ok(dir.pop().unwrap()),
                n => command_error(format!(
                    "Single path expected, found {} different paths.",
                    n
                )),
            }
        }
    }
}
