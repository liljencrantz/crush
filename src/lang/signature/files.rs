use crate::lang::data::binary::{BinaryReader, binary_channel};
use crate::lang::errors::{CrushError, CrushResult, command_error, data_error};
use crate::lang::pipe::{ValueReceiver, ValueSender};
use crate::lang::value::{Value, ValueType};
use crate::util::file::cwd;
use crate::util::regex::RegexFileMatcher;
use std::convert::TryFrom;
use std::fs::File;
use std::io::Write;
use std::ops::Deref;
use std::path::PathBuf;

/**
A type representing a set of files. It is used in the signature of builtin commands that
accept files, including globs, regexes, etc.
 */
#[derive(Debug, Clone)]
pub struct Files {
    had_entries: bool,
    files: Vec<PathBuf>,
}

impl From<Files> for Vec<PathBuf> {
    fn from(files: Files) -> Vec<PathBuf> {
        files.files
    }
}

impl TryFrom<Files> for PathBuf {
    type Error = CrushError;

    fn try_from(mut value: Files) -> CrushResult<PathBuf> {
        if value.files.len() == 1 {
            Ok(value.files.remove(0))
        } else {
            data_error("Invalid file")
        }
    }
}

impl Files {
    pub fn new() -> Files {
        Files {
            had_entries: false,
            files: Vec::new(),
        }
    }

    pub fn had_entries(&self) -> bool {
        self.had_entries
    }

    pub fn reader(self, input: ValueReceiver) -> CrushResult<Box<dyn BinaryReader + Send + Sync>> {
        if !self.had_entries {
            match input.recv()? {
                Value::BinaryInputStream(b) => Ok(b),
                Value::Binary(b) => Ok(<dyn BinaryReader>::vec(&b)),
                Value::String(s) => Ok(<dyn BinaryReader>::vec(s.as_bytes())),
                _ => command_error("Expected either a file to read or binary pipe io"),
            }
        } else {
            <dyn BinaryReader>::paths(self.files)
        }
    }

    pub fn writer(self, output: ValueSender) -> CrushResult<Box<dyn Write>> {
        if !self.had_entries {
            let (w, r) = binary_channel();
            output.send(Value::BinaryInputStream(r))?;
            Ok(w)
        } else if self.files.len() == 1 {
            output.send(Value::Empty)?;
            Ok(Box::from(File::create(self.files[0].clone())?))
        } else {
            command_error("Expected at most one destination file")
        }
    }

    pub fn expand(&mut self, value: Value) -> CrushResult<()> {
        match value {
            Value::File(p) => self.files.push(p.to_path_buf()),
            Value::Glob(pattern) => pattern.glob_files(&PathBuf::from("."), &mut self.files)?,
            Value::Regex(_, re) => re.match_files(&cwd()?, &mut self.files)?,
            Value::String(f) => self.files.push(PathBuf::from(f.deref())),
            value => {
                let mut input = value.stream()?;
                let types = input.types();
                if types.len() == 1 && types[0].cell_type == ValueType::File {
                    while let Ok(row) = input.read() {
                        if let Value::File(f) = Vec::from(row).remove(0) {
                            self.files.push(f.to_path_buf());
                        }
                    }
                } else {
                    return command_error("Table stream must contain one column of type file");
                }
            }
        }
        self.had_entries = true;
        Ok(())
    }
}
