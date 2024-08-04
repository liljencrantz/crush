use crate::lang::printer::Printer;
use regex::Regex;
use std::fs::read_dir;
use std::path::{Path, PathBuf};

pub trait RegexFileMatcher {
    fn match_files(&self, cwd: &Path, out: &mut Vec<PathBuf>, printer: &Printer);
}

impl RegexFileMatcher for Regex {
    fn match_files(&self, p: &Path, out: &mut Vec<PathBuf>, printer: &Printer) {
        match read_dir(p) {
            Ok(dir) => {
                for e in dir {
                    match e {
                        Ok(entry) => {
                            match entry.file_name().to_str() {
                                None => printer.error("Invalid filename encountered. Sadly, I cannot tell you what it is. Because it's invalid."),
                                Some(name) => {
                                    if self.is_match(name) {
                                        out.push(p.join(entry.file_name()));
                                    }
                                },
                            }
                        }
                        e => printer.handle_error(e.map_err({|ee| ee.into()})),
                    }
                }
            }
            e => printer.handle_error(e.map_err({|ee| ee.into()})),
        }
    }
}
