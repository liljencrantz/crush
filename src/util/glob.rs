use std::str::Chars;
use std::iter::Peekable;
use std::path::Path;
use std::io;
use std::fs::{read_dir, ReadDir};
use crate::lang::errors::{to_crush_error, argument_error, CrushResult};

#[derive(Clone)]
#[derive(PartialEq)]
#[derive(Eq)]
#[derive(Debug)]
#[derive(Hash)]
#[derive(PartialOrd)]
#[derive(Ord)]
pub struct Glob {
    pattern: String,
}

impl ToString for Glob {
    fn to_string(&self) -> String {
        self.pattern.clone()
    }
}

impl Glob {
    pub fn new(pattern: &str) -> Glob {
        Glob { pattern: pattern.to_string() }
    }

    pub fn matches(&self, v: &str) -> bool {
        glob_match(&mut self.pattern.chars(), &mut v.chars().peekable())
    }

    pub fn glob_files(&self, cwd: &Path, out: &mut Vec<Box<Path>>) -> CrushResult<()> {
        to_crush_error(Glob::glob_files_testable(self.pattern.as_str(), cwd, out,|p| read_dir(p)))
    }

    pub fn glob_to_single_file(&self, cwd: &Path) -> CrushResult<Box<Path>> {
        let mut dirs = Vec::new();
        self.glob_files(cwd, &mut dirs)?;
        match dirs.len() {
            1 => Ok(dirs.remove(0)),
            _ => argument_error("Glob expanded to wrong number of files"),
        }
    }

    fn glob(pattern: &str, v: &str) -> bool {
        glob_match(&mut pattern.chars(), &mut v.chars().peekable())
    }

    fn glob_files_testable(original_glob: &str, cwd: &Path, out: &mut Vec<Box<Path>>, lister: fn(&Path) -> io::Result<ReadDir>) -> io::Result<()> {
        let only_directories = original_glob.ends_with('/');
        let without_trailing_slashes = original_glob.trim_end_matches('/');
        if without_trailing_slashes.starts_with('/') {
            let without_leading_slashes = without_trailing_slashes.trim_start_matches('/');
            Glob::glob_files_internal(without_leading_slashes, Path::new("/"), only_directories, "/", out, lister)
        } else {
            Glob::glob_files_internal(without_trailing_slashes, cwd, only_directories, "", out, lister)
        }
    }

    fn glob_files_internal(
        relative_glob: &str,
        dir: &Path,
        only_directories: bool,
        prefix: &str,
        out: &mut Vec<Box<Path>>,
        lister: fn(&Path) -> io::Result<ReadDir>) -> io::Result<()> {
        let is_last_section = !relative_glob.contains('/');
        if is_last_section {
            for entry in lister(dir)? {
                let ee = entry?;
                match ee.file_name().to_str() {
                    Some(name) => {
                        if Glob::glob(relative_glob, name) && (!only_directories || ee.path().is_dir()) {
                            out.push(ee.path().into_boxed_path());
                        }
                    }
                    None => return Err(io::Error::new(io::ErrorKind::Other, "Invalid file name")),
                }
            }
        } else {
            let slash_idx = relative_glob.find('/').expect("impossible");
            let current_glob = &relative_glob[0..slash_idx];
            let next_glob = &relative_glob[slash_idx + 1..];
            for entry in read_dir(dir)? {
                let ee = entry?;
                match ee.file_name().to_str() {
                    Some(name) => {
                        if Glob::glob(current_glob, name) && (ee.path().is_dir()) {
                            Glob::glob_files_internal(next_glob, ee.path().as_path(), only_directories, format!("{}{}/", prefix, name).as_str(), out, lister)?;
                        }
                    }
                    None => return Err(io::Error::new(io::ErrorKind::Other, "Invalid file name")),
                }
            }
        }
        Ok(())
    }
}

fn glob_match(glob: &mut Chars, value: &mut Peekable<Chars>) -> bool {
    match (glob.next(), value.peek()) {
        (None, None) => return true,
        (None, Some(_)) => return false,
        (Some('%'), _) => {
            let mut i = value.clone();
            loop {
                match i.peek() {
                    Some(_) => {
                        if glob_match(&mut glob.clone(), &mut i.clone()) {
                            return true;
                        }
                        i.next();
                    }
                    None => {
                        if glob_match(&mut glob.clone(), &mut i.clone()) {
                            return true;
                        }
                        break;
                    }
                }
            }
        }
        (Some('?'), Some(_)) => {
            value.next();
            return glob_match(glob, value);
        }
        (Some(g), Some(v)) => {
            if g == *v {
                value.next();
                return glob_match(glob, value);
            }
        }
        (Some(_), None) => {}
    }
    false
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_that_globs_match_themselves() {
        assert!(Glob::new("foo.txt").matches("foo.txt"));
        assert!(Glob::new("").matches(""));
        assert!(!Glob::new("foo").matches("bar"));
    }

    #[test]
    fn test_that_basic_wildcards_work() {
        assert!(Glob::new("%.txt").matches("foo.txt"));
        assert!(!Glob::new("%.txt").matches("foo.txb"));
        assert!(!Glob::new("%.txt").matches("footxt"));
    }

    #[test]
    fn test_that_single_character_wildcards_work() {
        assert!(Glob::new("??.txt").matches("aa.txt"));
        assert!(!Glob::new("??.txt").matches("aaa.txt"));
        assert!(Glob::new("???").matches("aaa"));
        assert!(Glob::new("?").matches("a"));
    }

    #[test]
    fn test_that_wildcards_work_at_the_end() {
        assert!(Glob::new("%").matches("aaa"));
        assert!(Glob::new("aaa%").matches("aaa"));
        assert!(Glob::new("aaa%").matches("aaaa"));
        assert!(Glob::new("aaa%").matches("aaab"));
        assert!(Glob::new("aaa%?").matches("aaab"));
        assert!(Glob::new("aaa%?").matches("aaaab"));
        assert!(Glob::new("%a%").matches("aaaa"));
        assert!(!Glob::new("%a%").matches("bbb"));
    }

    #[test]
    fn test_that_multiple_wildcards_work() {
        assert!(Glob::new("a%b%c").matches("abc"));
        assert!(Glob::new("a%b%c?").matches("aabcc"));
        assert!(!Glob::new("a%b%c?").matches("acb"));
    }

//    #[test]
//    fn test_file_glob() -> io::Result<()> {
//        let mut out: Vec<String> = Vec::new();
//        glob_files("C*", Path::new("."), &mut out)?;
//        assert_eq!(out, vec!["Cargo.lock", "Cargo.toml"]);
//        return Ok(());
//    }
//
//    #[test]
//    fn test_subdirectory_glob() -> io::Result<()> {
//        let mut out: Vec<String> = Vec::new();
//        glob_files("s*/m*.rs", Path::new("."), &mut out)?;
//        assert_eq!(out, vec!["src/main.rs"]);
//        return Ok(());
//    }
//
//    #[test]
//    fn test_absolute_subdirectory_with_trailing_slash_glob() -> io::Result<()> {
//        let mut out: Vec<String> = Vec::new();
//        glob_files("/home/*/", Path::new("."), &mut out)?;
//        assert_eq!(out, vec!["/home/liljencrantz/"]);
//        return Ok(());
//    }
//
//    #[test]
//    fn test_absolute_subdirectory_glob() -> io::Result<()> {
//        let mut out: Vec<String> = Vec::new();
//        glob_files("/home/*", Path::new("."), &mut out)?;
//        assert_eq!(out, vec!["/home/liljencrantz"]);
//        return Ok(());
//    }
}
