use crate::lang::printer::Printer;
use regex::Regex;
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use crate::lang::errors;
use crate::lang::errors::{CrushError, CrushErrorType, CrushResult};

pub trait RegexFileMatcher {
    fn match_files(&self, cwd: &Path, out: &mut Vec<PathBuf>) -> CrushResult<()>;
}

impl RegexFileMatcher for Regex {
    fn match_files(&self, p: &Path, out: &mut Vec<PathBuf>) -> CrushResult<()> {
        match read_dir(p) {
            Ok(dir) => {
                for e in dir {
                    match e {
                        Ok(entry) => {
                            match entry.file_name().to_str() {
                                None => {
                                    return errors::error(
                                        "Invalid filename encountered. Sadly, I cannot tell you what it is. Because it's invalid.");
                                }
                                Some(name) => {
                                    if self.is_match(name) {
                                        out.push(p.join(entry.file_name()));
                                    }
                                },
                            }
                        }
                        Err(e) => {
                            return Err(CrushError::from(e));
                        }
                    }
                }
            }
            Err(e) => {
                return Err(CrushError::from(e));
            }
        }
        Ok(())
    }
}
