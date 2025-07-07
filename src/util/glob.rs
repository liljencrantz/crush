use crate::lang::errors::CrushResult;
use crate::util::directory_lister::{Directory, DirectoryLister, directory_lister};
use crate::util::file::home;
use crate::util::glob::CompileState::{
    InitialState, Regular, WasAny, WasDot, WasDotDot, WasSeparator,
};
use std::collections::{HashSet, VecDeque};
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

#[derive(Clone, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub struct Glob {
    original: String,
    pattern: Vec<Tile>,
}

#[derive(Clone, PartialEq, Eq, Debug, Hash, PartialOrd, Ord, Copy)]
enum Tile {
    Char(char),
    Single,
    Any,
    Recursive,
    Separator,
    Parent,
    Home,
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

#[derive(Clone, Copy)]
enum GlobMode {
    Glob,
    Complete,
}

enum CompileState {
    InitialState,
    WasAny,
    WasSeparator,
    WasDot,
    WasDotDot,
    Regular,
}

fn compile(s: &str) -> Vec<Tile> {
    let mut res = Vec::new();
    let mut state = InitialState;
    for c in s.chars() {
        match state {
            InitialState => {
                state = Regular;
                match c {
                    '~' => res.push(Tile::Home),
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

            Regular => match c {
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
                        res.push(Tile::Char('.'));
                        state = WasSeparator;
                    }
                    '?' => {
                        res.push(Tile::Char('.'));
                        res.push(Tile::Single)
                    }
                    '.' => state = WasDotDot,
                    c => {
                        res.push(Tile::Char('.'));
                        res.push(Tile::Char(c))
                    }
                }
            }
            WasDotDot => match c {
                '*' => {
                    res.push(Tile::Char('.'));
                    res.push(Tile::Char('.'));
                    state = WasAny
                }
                '/' => {
                    res.push(Tile::Parent);
                    state = WasSeparator;
                }
                '?' => {
                    res.push(Tile::Char('.'));
                    res.push(Tile::Char('.'));
                    res.push(Tile::Single);
                    state = Regular
                }
                c => {
                    res.push(Tile::Char('.'));
                    res.push(Tile::Char('.'));
                    res.push(Tile::Char(c));
                    state = Regular
                }
            },
        }
    }

    match state {
        InitialState => {}
        WasAny => res.push(Tile::Any),
        WasSeparator => {}
        Regular => {}
        WasDot => {}
        WasDotDot => res.push(Tile::Parent),
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
        self.glob_internal(cwd, out, GlobMode::Glob)
    }

    fn glob_internal(&self, cwd: &Path, out: &mut Vec<PathBuf>, mode: GlobMode) -> CrushResult<()> {
        match self.pattern.get(0) {
            Some(Tile::Separator) => glob_files(
                &self.pattern[1..],
                &PathBuf::from("/"),
                out,
                &directory_lister(),
                mode,
            ),

            Some(Tile::Home) => glob_files(
                &insert_home(&self.pattern, &home()?)?,
                &PathBuf::from("/"),
                out,
                &directory_lister(),
                mode,
            ),

            _ => glob_files(&self.pattern, cwd, out, &directory_lister(), mode),
        }
    }

    pub fn complete(&self, cwd: &Path, out: &mut Vec<String>) -> CrushResult<()> {
        let mut res = Vec::new();
        self.glob_internal(cwd, &mut res, GlobMode::Complete)?;
        let mut strs: Vec<_> = res
            .iter()
            .flat_map(|p| p.to_str().map(|pp| pp.to_string()))
            .collect();

        out.append(&mut strs);
        Ok(())
    }
}

struct GlobState<'s> {
    pattern: &'s [Tile],
    directory: PathBuf,
}

fn insert_home(glob: &[Tile], home: &Path) -> CrushResult<Vec<Tile>> {
    let mut res = Vec::new();
    for c in home.to_str().ok_or("Invalid home directory")?[1..].chars() {
        match c {
            '/' => res.push(Tile::Separator),
            cc => res.push(Tile::Char(cc)),
        }
    }
    res.extend_from_slice(&glob[1..]);
    Ok(res)
}

fn glob_file_match<'a>(
    pattern: &'a [Tile],
    path: &[char],
    entry: &Directory,
    out: &mut HashSet<PathBuf>,
    queue: &mut VecDeque<GlobState<'a>>,
    mode: GlobMode,
) -> CrushResult<()> {
    match (pattern.first(), path.first()) {
        (None, None) => match mode {
            GlobMode::Glob => {
                out.insert(entry.full_path.clone());
            }
            GlobMode::Complete => {}
        },
        (Some(Tile::Home), Some('~')) => {}
        (Some(Tile::Home), _) => {}
        (Some(Tile::Char(_)), None) | (Some(Tile::Single), None) => {}
        (Some(Tile::Any), None) => {
            if pattern.len() == 1 {
                match mode {
                    GlobMode::Glob => {
                        out.insert(entry.full_path.clone());
                    }
                    GlobMode::Complete => {}
                }
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

        (Some(Tile::Parent), _) => {
            // This should be impossible
        }

        (None, Some(_)) => match mode {
            GlobMode::Glob => {}
            GlobMode::Complete => {
                let mut str = path.iter().collect::<String>();
                if entry.is_directory {
                    str.push('/');
                }
                out.insert(PathBuf::from(str));
            }
        },

        (Some(Tile::Char(ch1)), Some(ch2)) if ch1 == ch2 => {
            return glob_file_match(&pattern[1..], &path[1..], entry, out, queue, mode);
        }
        (Some(Tile::Char(_)), Some(_)) => {}
        (Some(Tile::Single), Some(_)) => {
            return glob_file_match(&pattern[1..], &path[1..], entry, out, queue, mode);
        }
        (Some(Tile::Any), Some(_)) | (Some(Tile::Recursive), Some(_)) => {
            glob_file_match(&pattern[1..], path, entry, out, queue, mode)?;
            glob_file_match(pattern, &path[1..], entry, out, queue, mode)?;
        }
        (Some(Tile::Separator), Some(_)) => {}
    }
    Ok(())
}

fn glob_files(
    pattern: &[Tile],
    cwd: &Path,
    out: &mut Vec<PathBuf>,
    lister: &impl DirectoryLister,
    mode: GlobMode,
) -> CrushResult<()> {
    if pattern.is_empty() {
        return Ok(());
    }

    let mut queue = VecDeque::new();
    let mut dedup = HashSet::new();

    queue.push_back(if matches!(pattern[0], Tile::Char('/')) {
        GlobState {
            pattern: &pattern[1..],
            directory: PathBuf::from("/"),
        }
    } else {
        GlobState {
            pattern,
            directory: cwd.to_path_buf(),
        }
    });

    while let Some(state) = queue.pop_front() {
        if state.pattern.starts_with(&[Tile::Parent]) {
            state.directory.parent().inspect(|parent| {
                queue.push_back(GlobState {
                    pattern: &state.pattern[1..],
                    directory: parent.to_path_buf(),
                });
            });
        } else {
            for entry in lister.list(&state.directory)? {
                if let Some(path) = entry.name.to_str() {
                    let path_vec: Vec<char> = path.chars().collect();
                    glob_file_match(
                        state.pattern,
                        &path_vec,
                        &entry,
                        &mut dedup,
                        &mut queue,
                        mode,
                    )?;
                }
            }
        }
    }

    for e in dedup.drain() {
        out.push(e);
    }
    Ok(())
}

/// Match a glob against a given static string.
/// Internally, it recursively calls itself.
/// This is used for matching against given strings, but can't really be
/// used to match against files in a directory.
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
        Some(Tile::Parent) => {
            panic!("FIXME");
        }
        Some(Tile::Home) => match value.chars().next() {
            Some('~') => glob_match(&pattern[1..], &value[1..]),
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
    use super::*;
    use crate::util::directory_lister::tests::FakeDirectoryLister;
    use itertools::Itertools;

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
        res.add("tree", &vec!["a"]).add("tree/sub", &vec!["b", "c"]);
        res
    }

    fn check_file_glob_count(glob: &str, count: usize) {
        let mut out = Vec::new();
        let ggg = glob_files(
            &compile(glob),
            &PathBuf::from(""),
            &mut out,
            &lister(),
            GlobMode::Glob,
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
            GlobMode::Glob,
        );
        let set: HashSet<String> = out
            .drain(..)
            .map(|e| e.to_str().unwrap().to_string())
            .collect();
        assert_eq!(set.len(), matches.len());
        for el in matches {
            assert!(
                set.contains(*el),
                "The element '{}' wasn't present in glob result. The following items were found: {}",
                el,
                set.iter().map({ |e| format!("'{}'", e) }).join(", ")
            );
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
