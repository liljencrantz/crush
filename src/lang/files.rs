use std::path::PathBuf;
use crate::lang::printer::Printer;
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::value::{Value, ValueType};
use crate::util::file::cwd;
use crate::util::regex::RegexFileMatcher;
use crate::lang::binary::BinaryReader;
use crate::lang::stream::ValueReceiver;

#[derive(Debug)]
pub struct Files {
    had_entries: bool,
    files: Vec<PathBuf>,
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

    pub fn into_vec(self) -> Vec<PathBuf> {
        self.files
    }

    pub fn reader(self, input: ValueReceiver) -> CrushResult<Box<dyn BinaryReader + Send + Sync>> {
        if !self.had_entries {
            match input.recv()? {
                Value::BinaryStream(b) => Ok(b),
                Value::Binary(b) => Ok(BinaryReader::vec(&b)),
                _ => argument_error("Expected either a file to read or binary pipe input"),
            }
        } else {
            BinaryReader::paths(self.files)
        }
    }

    pub fn expand(&mut self, value: Value, printer: &Printer) -> CrushResult<()> {
        match value {
            Value::File(p) => self.files.push(p),
            Value::Glob(pattern) => pattern.glob_files(&PathBuf::from("."), &mut self.files)?,
            Value::Regex(_, re) => re.match_files(&cwd()?, &mut self.files, printer),
            value => {
                match value.readable() {
                    None => return argument_error("Expected a file name"),
                    Some(mut s) => {
                        let t = s.types();
                        if t.len() == 1 && t[0].cell_type == ValueType::File {
                            while let Ok(row) = s.read() {
                                if let Value::File(f) = row.into_vec().remove(0) {
                                    self.files.push(f);
                                }
                            }
                        } else {
                            return argument_error("Table stream must contain one column of type file");
                        }
                    }
                }
            }
        }
        self.had_entries = true;
        Ok(())
    }
}


