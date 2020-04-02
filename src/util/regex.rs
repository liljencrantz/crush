use regex::Regex;
use std::path::Path;
use crate::lang::errors::to_crush_error;
use std::fs::read_dir;
use crate::lang::printer::printer;

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
                                None => printer().error("Invalid filename encountered. Sadly, I cannot tell you what it is. Because it's invalid."),
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
