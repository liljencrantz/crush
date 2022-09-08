use crate::lang::errors::{CrushResult, data_error};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::fmt::{Display, Formatter};
use crate::util::directory_lister::{directory_lister, DirectoryLister};

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

impl Display for Glob {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.original)
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
        glob_files(&self.pattern, cwd, out, &directory_lister())
    }
}

fn glob_files(pattern: &[Tile], cwd: &Path, out: &mut Vec<PathBuf>, lister: &impl DirectoryLister) -> CrushResult<()> {
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
        for entry in lister.list(&next_dir)? {
            match entry.name.to_str() {
                Some(name) => {
                    let mut ss = format!("{}{}", s, name);
                    let res = glob_match(pattern, &ss);
                    if res.matches {
                        out.push(PathBuf::from(&ss))
                    }
                    if res.prefix && entry.is_directory {
                        if !res.matches {
                            let with_trailing_slash = format!("{}/", ss);
                            if glob_match(pattern, &with_trailing_slash).matches {
                                out.push(PathBuf::from(&with_trailing_slash))
                            }
                        }
                        ss.push('/');
                        queue.push_back((ss, entry.full_path));
                    }
                }
                None => return data_error("Invalid file name"),
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
    use crate::util::directory_lister::FakeDirectoryLister;

    #[test]
    fn test_glob_match() {
        assert_eq!(
            glob_match(&compile("%%"), "a"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("%%"), "a/b/c/d"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("%%"), "a/b/c/d/"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("%%/"), "a/"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("%%/"), "a/b/c/d/"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("%%/"), "a"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("%%/"), "a/b/c/d"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("%%a"), "aaa"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("%%a/"), "aaa/"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("%%a"), "aaa/"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("aaa/%"), "aaa"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("a/%/c"), "a/bbbb"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("?"), "a"),
            GlobResult {
                matches: true,
                prefix: false,
            }
        );
        assert_eq!(
            glob_match(&compile("a/"), "a"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("?/"), "a"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("a/?/c"), "a/b"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("a/?/c"), "a/bb"),
            GlobResult {
                matches: false,
                prefix: false,
            }
        );
        assert_eq!(
            glob_match(&compile("%%a"), "bbb"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("%"), "a/b"),
            GlobResult {
                matches: false,
                prefix: false,
            }
        );
        assert_eq!(
            glob_match(&compile("%%c"), "a/b"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("a/%/c"), "a/b/c"),
            GlobResult {
                matches: true,
                prefix: false,
            }
        );
        assert_eq!(
            glob_match(&compile("a/b%/c"), "a/b/c"),
            GlobResult {
                matches: true,
                prefix: false,
            }
        );
        assert_eq!(
            glob_match(&compile("a/%b/c"), "a/d/c"),
            GlobResult {
                matches: false,
                prefix: false,
            }
        );
        assert_eq!(
            glob_match(&compile("a/%/c/"), "a/b/c/"),
            GlobResult {
                matches: true,
                prefix: false,
            }
        );
    }

    fn lister() -> FakeDirectoryLister {
        let mut res = FakeDirectoryLister::new("/home/rabbit");
        res.add("example_data/tree", &vec!["a"])
            .add("example_data/tree/sub", &vec!["b", "c"]);
        res
    }

    #[test]
    fn test_glob_files() {
        let mut out = Vec::new();
        let _ = glob_files(
            &compile("%%"),
            &PathBuf::from("example_data/tree"),
            &mut out,
            &lister(),
        );
        assert_eq!(out.len(), 4);
        out.clear();
        let _ = glob_files(
            &compile("%%/"),
            &PathBuf::from("example_data/tree"),
            &mut out,
            &lister(),
        );
        assert_eq!(out.len(), 1);
        out.clear();
        let _ = glob_files(
            &compile("./tree/s%"),
            &PathBuf::from("example_data"),
            &mut out,
            &lister(),
        );
        assert_eq!(out.len(), 1);
        out.clear();
        let _ = glob_files(
            &compile("%%/%"),
            &PathBuf::from("example_data/tree"),
            &mut out,
            &lister(),
        );
        assert_eq!(out.len(), 3);
        out.clear();
        let _ = glob_files(
            &compile("?%%/?"),
            &PathBuf::from("example_data/tree"),
            &mut out,
            &lister(),
        );
        assert_eq!(out.len(), 2);
        out.clear();
        let _ = glob_files(
            &compile("%%b"),
            &PathBuf::from("example_data/tree"),
            &mut out,
            &lister(),
        );
        assert_eq!(out.len(), 2);
    }
}
