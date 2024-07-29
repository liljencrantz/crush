use crate::lang::errors::{CrushResult, data_error};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::fmt::{Display, Formatter};
use crate::util::directory_lister::{Directory, directory_lister, DirectoryLister};
use crate::util::glob::CompileState::{Regular, WasAny, WasDot, WasSeparator};

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
    Separator,
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

enum CompileState {
    WasAny,
    WasSeparator,
    WasDot,
    Regular,
}

fn compile(s: &str) -> Vec<Tile> {
    let mut res = Vec::new();
    let mut state = WasSeparator;
    for c in s.chars() {
        match state {
            Regular =>
                match c {
                    '*' => state = WasAny,
                    '/' => {
                        state = WasSeparator;
                        res.push(Tile::Separator)
                    }
                    '?' => res.push(Tile::Single),
                    c => res.push(Tile::Char(c)),
                },
            WasAny => {
                state = Regular;
                match c {
                    '*' => res.push(Tile::Recursive),
                    '?' => {
                        res.push(Tile::Any);
                        res.push(Tile::Single);
                    }
                    '/' => {
                        state = WasSeparator;
                        res.push(Tile::Any);
                        res.push(Tile::Separator);
                    }
                    c => {
                        res.push(Tile::Any);
                        res.push(Tile::Char(c));
                    }
                }
            }
            WasSeparator => {
                state = Regular;
                match c {
                    '.' => state = WasDot,
                    '*' => state = WasAny,
                    '/' => {
                        state = WasSeparator;
                        res.push(Tile::Separator)
                    }
                    '?' => res.push(Tile::Single),
                    c => res.push(Tile::Char(c)),
                }
            }
            WasDot => {
                state = Regular;
                match c {
                    '*' => {
                        res.push(Tile::Char('.'));
                        state = WasAny
                    }
                    '/' => {
                        state = WasSeparator;
                    }
                    '?' => {
                        res.push(Tile::Char('.'));
                        res.push(Tile::Single)
                    }
                    c => {
                        res.push(Tile::Char('.'));
                        res.push(Tile::Char(c))
                    }
                }
            }
        }
    }

    match state {
        WasAny => res.push(Tile::Any),
        WasSeparator => {}
        Regular => {}
        WasDot => {}
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

struct GlobState<'s> {
    pattern: &'s [Tile],
    directory: PathBuf,
}

enum GlobMatchResult<'s> {
    FullMatch,
    PartialMatch { remaining_pattern: &'s [Tile] },
}

fn glob_file_match<'a>(pattern: &'a [Tile], path: &[char], entry: &Directory, out: &mut HashSet<PathBuf>, queue: &mut VecDeque<GlobState<'a>>) -> CrushResult<()> {
    match (pattern.first(), path.first()) {
        (None, None) => {
            out.insert(entry.full_path.clone());
        }
        (Some(Tile::Char(_)), None) | (Some(Tile::Single), None) => {}
        (Some(Tile::Any), None) => {
            if pattern.len() == 1 {
                out.insert(entry.full_path.clone());
            }
            if entry.is_directory {
                if pattern[1..] == [Tile::Separator] {
                    out.insert(entry.full_path.clone());
                }

                if let Some(Tile::Separator) = pattern.get(1) {
                    queue.push_back(GlobState {
                        pattern: &pattern[2..],
                        directory: entry.full_path.clone(),
                    });
                }
            }
        }
        (Some(Tile::Recursive), None) => {
            if pattern.len() == 1 {
                out.insert(entry.full_path.clone());
            }
            if entry.is_directory {
                if pattern[1..] == [Tile::Separator] {
                    out.insert(entry.full_path.clone());
                }

                if let Some(Tile::Separator) = pattern.get(1) {
                    queue.push_back(GlobState {
                        pattern: &pattern[2..],
                        directory: entry.full_path.clone(),
                    });
                }

                queue.push_back(GlobState {
                    pattern,
                    directory: entry.full_path.clone(),
                });
            }
        }
        (Some(Tile::Separator), None) => {
            if pattern.len() == 1 {
                if entry.is_directory {
                    out.insert(entry.full_path.clone());
                }
            } else {
                if entry.is_directory {
                    queue.push_back(GlobState {
                        pattern: &pattern[1..],
                        directory: entry.full_path.clone(),
                    });
                }
            }
        }

        (None, Some(ch)) => {}
        (Some(Tile::Char(ch1)), Some(ch2)) if ch1 == ch2 => {
            return glob_file_match(&pattern[1..], &path[1..], entry, out, queue)
        }
        (Some(Tile::Char(_)), Some(_)) => {}
        (Some(Tile::Single), Some(_)) => {
            return glob_file_match(&pattern[1..], &path[1..], entry, out, queue)
        }
        (Some(Tile::Any), Some(_)) | (Some(Tile::Recursive), Some(_)) => {
            glob_file_match(&pattern[1..], path, entry, out, queue)?;
            glob_file_match(pattern, &path[1..], entry, out, queue)?;
        }
        (Some(Tile::Separator), Some(_)) => {}
    }
    Ok(())
}

fn glob_files(pattern: &[Tile], cwd: &Path, out: &mut Vec<PathBuf>, lister: &impl DirectoryLister) -> CrushResult<()> {
    if pattern.is_empty() {
        return Ok(());
    }

    let mut queue = VecDeque::new();
    let mut dedup = HashSet::new();

    queue.push_back(if matches!(pattern[0], Tile::Char('/')) {
        GlobState { pattern: &pattern[1..], directory: PathBuf::from("/") }
    } else {
        GlobState { pattern, directory: cwd.to_path_buf() }
    });

    while let Some(state) = queue.pop_front() {
        for entry in lister.list(&state.directory)? {
            if let Some(path) = entry.name.to_str() {
                let path_vec: Vec<char> = path.chars().collect();
                glob_file_match(state.pattern, &path_vec, &entry, &mut dedup, &mut queue);
            }
        }
    }

    for e in dedup.drain() {
        out.push(e);
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

        Some(Tile::Separator) => match value.chars().next() {
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
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use super::*;
    use crate::util::directory_lister::tests::FakeDirectoryLister;

    #[test]
    fn test_glob_match() {
        assert_eq!(
            glob_match(&compile("**"), "a"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("**"), "a/b/c/d"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("**"), "a/b/c/d/"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("**/"), "a/"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("**/"), "a/b/c/d/"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("**/"), "a"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("**/"), "a/b/c/d"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("**a"), "aaa"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("**a/"), "aaa/"),
            GlobResult {
                matches: true,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("**a"), "aaa/"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("aaa/*"), "aaa"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("a/*/c"), "a/bbbb"),
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
            glob_match(&compile("**a"), "bbb"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("*"), "a/b"),
            GlobResult {
                matches: false,
                prefix: false,
            }
        );
        assert_eq!(
            glob_match(&compile("**c"), "a/b"),
            GlobResult {
                matches: false,
                prefix: true,
            }
        );
        assert_eq!(
            glob_match(&compile("a/*/c"), "a/b/c"),
            GlobResult {
                matches: true,
                prefix: false,
            }
        );
        assert_eq!(
            glob_match(&compile("a/b*/c"), "a/b/c"),
            GlobResult {
                matches: true,
                prefix: false,
            }
        );
        assert_eq!(
            glob_match(&compile("a/*b/c"), "a/d/c"),
            GlobResult {
                matches: false,
                prefix: false,
            }
        );
        assert_eq!(
            glob_match(&compile("a/*/c/"), "a/b/c/"),
            GlobResult {
                matches: true,
                prefix: false,
            }
        );
    }

    fn lister() -> FakeDirectoryLister {
        let mut res = FakeDirectoryLister::new("/home/rabbit");
        res.add("tree", &vec!["a"])
            .add("tree/sub", &vec!["b", "c"]);
        res
    }

    fn check_file_glob_count(glob: &str, count: usize) {
        let mut out = Vec::new();
        let ggg = glob_files(
            &compile(glob),
            &PathBuf::from(""),
            &mut out,
            &lister(),
        );
        assert_eq!(out.len(), count);
    }

    fn check_file_glob(glob: &str, matches: &[&str]) {
        let mut out = Vec::new();
        let ggg = glob_files(
            &compile(glob),
            &PathBuf::from(""),
            &mut out,
            &lister(),
        );
        let set: HashSet<String> = out.drain(..).map(|e|{e.to_str().unwrap().to_string()}).collect();
        assert_eq!(set.len(), matches.len());
        for el in matches {
            assert!(
                set.contains(*el),
                "The element '{}' wasn't present in glob result. The following items were found: {}",
                el,
                set.iter().map({|e| format!("'{}'", e)}).join(", "));
        }
    }

    #[test]
    fn test_glob_files_literal() {
        check_file_glob("tree", &["tree"]);
    }
    #[test]
    fn test_glob_files_single() {
        check_file_glob("tre?", &["tree"]);
    }
    #[test]
    fn test_glob_files_any() {
        check_file_glob("*", &["tree"]);
    }
    #[test]
    fn test_glob_files_recursive() {
        check_file_glob_count("**", 5);
    }
    #[test]
    fn test_glob_files_recursive_directories() {
        check_file_glob("**/", &["tree", "tree/sub"]);
    }

    #[test]
    fn test_glob_files_with_separator() {
        check_file_glob("tree/s*", &["tree/sub"]);
    }

    #[test]
    fn test_glob_files_leading_dot() {
        check_file_glob("./tree/s*", &["tree/sub"]);
    }

    #[test]
    fn test_glob_files_recursive_any() {
        check_file_glob_count("**/*", 4);
    }

    #[test]
    fn test_glob_files_recursive_any_directory() {
        check_file_glob("**/*/", &["tree/sub"]);
    }

    #[test]
    fn test_glob_files_single_recursive_any() {
        check_file_glob_count("?**/*", 4);
    }

    #[test]
    fn test_glob_files_trailing_letter() {
        check_file_glob("**b", &["tree/sub/b", "tree/sub"]);
    }
}
