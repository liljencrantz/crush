use regex::Regex;
use std::path::Path;
use crate::lang::errors::{CrushResult, CrushError, to_crush_error, mandate};
use std::fs::{read_dir, ReadDir};
use crate::lang::printer::printer;
use std::io::Error;


pub trait RegexFileMatcher {
    fn match_files(&self, cwd: &Path, out: &mut Vec<Box<Path>>);
}

impl RegexFileMatcher for Regex {
    fn match_files(&self, p: &Path, out: &mut Vec<Box<Path>>) {
        match read_dir(p) {
            Ok(dir) => {
                for e in dir {
                    match e {
                        Ok(entry) => {

                            match entry.file_name().to_str() {
                                None => printer().error("Invalid filename encoundered"),
                                Some(name) => {
                                    if self.is_match(name) {
                                        out.push(p.join(entry.file_name()).into_boxed_path());
                                    }
                                },
                            }
                        }
                        e => printer().handle_error(to_crush_error(e)),
                    }
                }
            },
            e => printer().handle_error(to_crush_error(e)),
        }
    }
}
