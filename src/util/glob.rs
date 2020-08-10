use crate::lang::errors::{argument_error, to_crush_error, CrushResult};
use std::collections::VecDeque;
use std::fs::read_dir;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub struct Glob {
    original: String,
    pattern: Vec<Tile>,
}

#[derive(Clone, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
enum Tile {
    Char(char),
    Single,
    Any,
    Recursive,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
struct GlobResult {
    matches: bool,
    prefix: bool,
}

impl ToString for Glob {
    fn to_string(&self) -> String {
        self.original.clone()
    }
}

fn compile(s: &str) -> Vec<Tile> {
    let mut res = Vec::new();
    let mut was_any = false;
    for c in s.chars() {
        if was_any {
            match c {
                '%' => res.push(Tile::Recursive),
                '?' => {
                    res.push(Tile::Any);
                    res.push(Tile::Single);
                }
                c => {
                    res.push(Tile::Any);
                    res.push(Tile::Char(c));
                }
            }
            was_any = false;
        } else {
            match c {
                '%' => was_any = true,
                '?' => {
                    res.push(Tile::Single);
                }
                c => {
                    res.push(Tile::Char(c));
                }
            }
        }
    }
    if was_any {
        res.push(Tile::Any);
    }
    res
}

impl Glob {
    pub fn new(def: &str) -> Glob {
        Glob {
            original: def.to_string(),
            pattern: compile(def),
        }
    }

    pub fn matches(&self, v: &str) -> bool {
        glob_match(&self.pattern, v).matches
    }

    pub fn glob_files(&self, cwd: &Path, out: &mut Vec<PathBuf>) -> CrushResult<()> {
        to_crush_error(glob_files(&self.pattern, cwd, out))
    }

    pub fn glob_to_single_file(&self, cwd: &Path) -> CrushResult<PathBuf> {
        let mut dirs = Vec::new();
        self.glob_files(cwd, &mut dirs)?;
        match dirs.len() {
            1 => Ok(dirs.remove(0)),
            _ => argument_error("Glob expanded to wrong number of files"),
        }
    }
}

fn glob_files(pattern: &[Tile], cwd: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    if pattern.is_empty() {
        return Ok(());
    }

    let mut queue = VecDeque::new();

    queue.push_back(if matches!(pattern[0], Tile::Char('/')) {
        ("/".to_string(), PathBuf::from("/"))
    } else {
        ("".to_string(), cwd.to_path_buf())
    });

    while !queue.is_empty() {
        let (s, next_dir) = queue.pop_front().unwrap();
        for entry in read_dir(&next_dir)? {
            let entry = entry?;
            match entry.file_name().to_str() {
                Some(name) => {
                    let mut ss = format!("{}{}", s, name);
                    let res = glob_match(pattern, &ss);
                    if res.matches {
                        out.push(PathBuf::from(&ss))
                    }
                    if res.prefix && entry.metadata()?.is_dir() {
                        if !res.matches {
                            let with_trailing_slash = format!("{}/", ss);
                            if glob_match(pattern, &with_trailing_slash).matches {
                                out.push(PathBuf::from(&with_trailing_slash))
                            }
                        }
                        ss.push('/');
                        queue.push_back((ss, entry.path()));
                    }
                }
                None => return Err(io::Error::new(io::ErrorKind::Other, "Invalid file name")),
            }
        }
    }
    Ok(())
}

fn glob_match(pattern: &[Tile], value: &str) -> GlobResult {
    let tile = pattern.first();
    match &tile {
        Some(Tile::Recursive) => match value.chars().next() {
            Some(_) => {
                let r = glob_match(&pattern[1..], value);
                if r.matches {
                    GlobResult {
                        matches: true,
                        prefix: true,
                    }
                } else {
                    glob_match(pattern, &value[1..])
                }
            }
            None => GlobResult {
                matches: pattern.len() == 1,
                prefix: true,
            },
        },

        Some(Tile::Any) => match value.chars().next() {
            Some('/') => glob_match(&pattern[1..], &value),
            Some(_) => {
                let r = glob_match(&pattern[1..], value);
                if r.matches {
                    r
                } else {
                    glob_match(pattern, &value[1..])
                }
            }
            None => GlobResult {
                matches: pattern.len() == 1,
                prefix: true,
            },
        },

        None => match value.chars().next() {
            None => GlobResult {
                matches: true,
                prefix: false,
            },
            Some(_) => GlobResult {
                matches: false,
                prefix: false,
            },
        },

        Some(Tile::Single) => match value.chars().next() {
            Some('/') => GlobResult {
                matches: false,
                prefix: false,
            },
            Some(_) => glob_match(&pattern[1..], &value[1..]),
            None => GlobResult {
                matches: false,
                prefix: false,
            },
        },

        Some(Tile::Char('/')) => match value.chars().next() {
            Some('/') => glob_match(&pattern[1..], &value[1..]),
            Some(_) => GlobResult {
                matches: false,
                prefix: false,
            },
            None => GlobResult {
                matches: false,
                prefix: true,
            },
        },

        Some(Tile::Char(g)) => match value.chars().next() {
            Some(v) => {
                if *g == v {
                    glob_match(&pattern[1..], &value[1..])
                } else {
                    GlobResult {
                        matches: false,
                        prefix: false,
                    }
                }
            }

            None => GlobResult {
                matches: false,
                prefix: false,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match() {
        assert_eq!(
            glob_match(&compile("%%"), "a"),
            GlobResult {
                matches: true,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("%%"), "a/b/c/d"),
            GlobResult {
                matches: true,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("%%"), "a/b/c/d/"),
            GlobResult {
                matches: true,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("%%/"), "a/"),
            GlobResult {
                matches: true,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("%%/"), "a/b/c/d/"),
            GlobResult {
                matches: true,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("%%/"), "a"),
            GlobResult {
                matches: false,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("%%/"), "a/b/c/d"),
            GlobResult {
                matches: false,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("%%a"), "aaa"),
            GlobResult {
                matches: true,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("%%a/"), "aaa/"),
            GlobResult {
                matches: true,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("%%a"), "aaa/"),
            GlobResult {
                matches: false,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("aaa/%"), "aaa"),
            GlobResult {
                matches: false,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("a/%/c"), "a/bbbb"),
            GlobResult {
                matches: false,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("?"), "a"),
            GlobResult {
                matches: true,
                prefix: false
            }
        );
        assert_eq!(
            glob_match(&compile("a/"), "a"),
            GlobResult {
                matches: false,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("?/"), "a"),
            GlobResult {
                matches: false,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("a/?/c"), "a/b"),
            GlobResult {
                matches: false,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("a/?/c"), "a/bb"),
            GlobResult {
                matches: false,
                prefix: false
            }
        );
        assert_eq!(
            glob_match(&compile("%%a"), "bbb"),
            GlobResult {
                matches: false,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("%"), "a/b"),
            GlobResult {
                matches: false,
                prefix: false
            }
        );
        assert_eq!(
            glob_match(&compile("%%c"), "a/b"),
            GlobResult {
                matches: false,
                prefix: true
            }
        );
        assert_eq!(
            glob_match(&compile("a/%/c"), "a/b/c"),
            GlobResult {
                matches: true,
                prefix: false
            }
        );
        assert_eq!(
            glob_match(&compile("a/b%/c"), "a/b/c"),
            GlobResult {
                matches: true,
                prefix: false
            }
        );
        assert_eq!(
            glob_match(&compile("a/%b/c"), "a/d/c"),
            GlobResult {
                matches: false,
                prefix: false
            }
        );
        assert_eq!(
            glob_match(&compile("a/%/c/"), "a/b/c/"),
            GlobResult {
                matches: true,
                prefix: false
            }
        );
    }

    #[test]
    fn test_glob_files() {
        let mut out = Vec::new();
        let _ = glob_files(
            &compile("%%"),
            &PathBuf::from("example_data/tree"),
            &mut out,
        );
        assert_eq!(out.len(), 4);
        out.clear();
        let _ = glob_files(
            &compile("%%/"),
            &PathBuf::from("example_data/tree"),
            &mut out,
        );
        assert_eq!(out.len(), 1);
        out.clear();
        let _ = glob_files(
            &compile("%%/%"),
            &PathBuf::from("example_data/tree"),
            &mut out,
        );
        assert_eq!(out.len(), 3);
        out.clear();
        let _ = glob_files(
            &compile("?%%/?"),
            &PathBuf::from("example_data/tree"),
            &mut out,
        );
        assert_eq!(out.len(), 2);
        out.clear();
        let _ = glob_files(
            &compile("%%b"),
            &PathBuf::from("example_data/tree"),
            &mut out,
        );
        assert_eq!(out.len(), 2);
    }
}
