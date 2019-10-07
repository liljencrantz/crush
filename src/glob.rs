use std::str::Chars;
use std::iter::Peekable;
use std::path::Path;
use std::io;
use std::fs::read_dir;

pub fn glob(g: &str, v: &str) -> bool {
    return glob_match(&mut g.chars(), &mut v.chars().peekable());
}

pub fn glob_files(original_glob: &str, cwd: &Path, out: &mut Vec<String>) -> io::Result<()> {
    let only_directories = original_glob.ends_with('/');
    let mut without_trailing_slashes = &original_glob[..];
    while without_trailing_slashes.ends_with('/') {
        without_trailing_slashes = &without_trailing_slashes[0..without_trailing_slashes.len() - 1];
    }

    if without_trailing_slashes.starts_with('/') {
        let mut without_leading_slashes = &without_trailing_slashes[..];
        while without_leading_slashes.starts_with('/') {
            without_leading_slashes = &without_leading_slashes[1..];
        }
        return glob_files_internal(without_leading_slashes, Path::new("/"), only_directories, "/", out);
    } else {
        return glob_files_internal(without_trailing_slashes, cwd, only_directories, "", out);
    }
}

pub fn glob_files_internal(relative_glob: &str, dir: &Path, only_directories: bool, prefix: &str, out: &mut Vec<String>) -> io::Result<()> {
    let is_last_section = !relative_glob.contains('/');
    if is_last_section {
        let suffix = if only_directories { "/" } else { "" };
        for entry in read_dir(dir)? {
            let ee = entry?;
            match ee.file_name().to_str() {
                Some(name) => {
                    if glob(relative_glob, name) && (!only_directories || ee.path().is_dir()) {
                        out.push(format!("{}{}{}", prefix, name, suffix));
                    }
                }
                None => panic!("OOOOO")
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
                    if glob(current_glob, name) && (ee.path().is_dir()) {
                        glob_files_internal(next_glob, ee.path().as_path(), only_directories, format!("{}{}/", prefix, name).as_str(), out)?;
                    }
                }
                None => panic!("OOOOO")
            }
        }
    }
    return Ok(());
}

fn glob_match(glob: &mut Chars, value: &mut Peekable<Chars>) -> bool {
    match (glob.next(), value.peek()) {
        (None, None) => return true,
        (None, Some(_)) => return false,
        (Some('*'), _) => {
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
        (Some('?'), Some(v)) => {
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
    return false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_that_globs_match_themselves() {
        assert!(glob("foo.txt", "foo.txt"));
        assert!(glob("", ""));
        assert!(!glob("foo", "bar"));
    }

    #[test]
    fn test_that_basic_wildcards_work() {
        assert!(glob("*.txt", "foo.txt"));
        assert!(!glob("*.txt", "foo.txb"));
        assert!(!glob("*.txt", "footxt"));
    }

    #[test]
    fn test_that_single_character_wildcards_work() {
        assert!(glob("??.txt", "aa.txt"));
        assert!(!glob("??.txt", "aaa.txt"));
        assert!(glob("???", "aaa"));
        assert!(glob("?", "a"));
    }

    #[test]
    fn test_that_wildcards_work_at_the_end() {
        assert!(glob("*", "aaa"));
        assert!(glob("aaa*", "aaa"));
        assert!(glob("aaa*", "aaaa"));
        assert!(glob("aaa*", "aaab"));
        assert!(glob("aaa*?", "aaab"));
        assert!(glob("aaa*?", "aaaab"));
        assert!(glob("*a*", "aaaa"));
        assert!(!glob("*a*", "bbb"));
    }

    #[test]
    fn test_that_multiple_wildcards_work() {
        assert!(glob("a*b*c", "abc"));
        assert!(glob("a*b*c?", "aabcc"));
        assert!(!glob("a*b*c?", "acb"));
    }

    #[test]
    fn test_file_glob() {
        let mut out: Vec<String> = Vec::new();
        glob_files("C*", Path::new("."), &mut out);
        assert_eq!(out, vec!["Cargo.lock", "Cargo.toml"]);
    }

    #[test]
    fn test_subdirectory_glob() {
        let mut out: Vec<String> = Vec::new();
        glob_files("s*/m*.rs", Path::new("."), &mut out);
        assert_eq!(out, vec!["src/main.rs"]);
    }

    #[test]
    fn test_absolute_subdirectory_with_trailing_slash_glob() {
        let mut out: Vec<String> = Vec::new();
        glob_files("/home/*/", Path::new("."), &mut out);
        assert_eq!(out, vec!["/home/liljencrantz/"]);
    }

    #[test]
    fn test_absolute_subdirectory_glob() {
        let mut out: Vec<String> = Vec::new();
        glob_files("/home/*", Path::new("."), &mut out);
        assert_eq!(out, vec!["/home/liljencrantz"]);
    }
}
