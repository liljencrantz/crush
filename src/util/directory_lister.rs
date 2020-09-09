/**
A simple wrapper around std::fs::read_dir to allow for unit testing via fakes.
*/

use std::path::{Path, PathBuf};
use crate::lang::errors::{CrushResult, to_crush_error, mandate};
use std::fs::{ReadDir, read_dir};
use ordered_map::{OrderedMap, Entry};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Directory {
    pub path: PathBuf,
    pub is_directory: bool,
}

pub trait DirectoryLister {
    type DirectoryIter: Iterator<Item=Directory>;

    fn list(&self, path: impl Into<PathBuf>) -> CrushResult<Self::DirectoryIter>;
}

pub struct RealDirectoryLister {}

impl RealDirectoryLister {
    pub fn new() -> RealDirectoryLister {
        RealDirectoryLister {}
    }
}

impl DirectoryLister for RealDirectoryLister {
    type DirectoryIter = RealIter;

    fn list(&self, path: impl Into<PathBuf>) -> CrushResult<RealIter> {
        Ok(
            RealIter {
                read_dir: to_crush_error(read_dir(&path.into()))?,
            }
        )
    }
}

pub struct RealIter {
    read_dir: ReadDir,
}

impl Iterator for RealIter {
    type Item = Directory;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            /*
                Loop on failure (to skip directories we're not allowed to read) and
                return None when read_dir returns None to terminate iteration
             */
            if let Ok(next) = self.read_dir.next()? {
                return Some(Directory {
                    path: PathBuf::from(next.file_name()),
                    is_directory: next.metadata().map(|m| m.is_dir()).unwrap_or(false),
                });
            }
        }
    }
}

pub struct FakeDirectoryLister {
    cwd: PathBuf,
    map: OrderedMap<PathBuf, Vec<Directory>>,
}

impl FakeDirectoryLister {
    pub fn new(cwd: impl Into<PathBuf>) -> FakeDirectoryLister {
        FakeDirectoryLister {
            map: OrderedMap::new(),
            cwd: cwd.into(),
        }
    }

    pub fn add(&mut self, path: impl Into<PathBuf>, content: &[&str]) -> &mut FakeDirectoryLister {
        let g = path.into();
        let path = if g.is_relative() {
            self.cwd.join(g)
        } else {
            g
        };

        let mut content = content.iter().map(|n| Directory { path: PathBuf::from(n), is_directory: false }).collect::<Vec<_>>();

        match self.map.entry(path.clone()) {
            Entry::Occupied(mut e) => {
                content.append(&mut e.value().clone());
                e.insert(content);
            }
            Entry::Vacant(e) => { e.insert(content.to_vec()) }
        }

        let mut parent = PathBuf::from(path);
        while let Some(p) = parent.parent() {
            let mut v = vec![
                Directory {
                    path: PathBuf::from(parent.components().last().unwrap().as_os_str()),
                    is_directory: true,
                }];

            match self.map.entry(p.to_path_buf()) {
                Entry::Occupied(mut e) => {
                    if !e.value().contains(&v[0]) {
                        let mut tmp = e.value().clone();
                        tmp.append(&mut v);
                        e.insert(tmp);
                    }
                }
                Entry::Vacant(e) => {
                    e.insert(v);
                }
            }

            parent = p.to_path_buf();
        }
        self
    }
}

impl DirectoryLister for FakeDirectoryLister {
    type DirectoryIter = FakeIter;

    fn list(&self, path: impl Into<PathBuf>) -> CrushResult<Self::DirectoryIter> {
        let mut g = path.into();
        let path = if g.is_relative() {
            self.cwd.join(g)
        } else {
            g
        };

        Ok(
            FakeIter {
                vec: VecDeque::from(mandate(self.map.get(&path), "Unknown directory")?.clone()),
                idx: 0,
            }
        )
    }
}

pub struct FakeIter {
    vec: VecDeque<Directory>,
    idx: usize,
}

impl Iterator for FakeIter {
    type Item = Directory;

    fn next(&mut self) -> Option<Self::Item> {
        self.vec.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_strs(it: FakeIter) -> Vec<String> {
        let mut res = it.map(|d| d.path.to_str().unwrap().to_string()).collect::<Vec<_>>();
        res.sort();
        res
    }

    fn as_strs_real(it: RealIter) -> Vec<String> {
        let mut res = it.map(|d| d.path.to_str().unwrap().to_string()).collect::<Vec<_>>();
        res.sort();
        res
    }

    #[test]
    fn check_fake() {
        let mut f = FakeDirectoryLister::new("/home/rabbit");
        f.add("a", &vec!["foo", "bar"]);
        f.add("a/baz", &vec!["qux", "pix"]);
        assert_eq!(as_strs(f.list("/home").unwrap()), vec!["rabbit"]);
        assert_eq!(as_strs(f.list("/home/").unwrap()), vec!["rabbit"]);
        assert_eq!(as_strs(f.list("/home/rabbit").unwrap()), vec!["a"]);
        assert_eq!(as_strs(f.list(".").unwrap()), vec!["a"]);
        assert_eq!(as_strs(f.list("/home/rabbit/a").unwrap()), vec!["bar", "baz", "foo"]);
        assert_eq!(as_strs(f.list("a").unwrap()), vec!["bar", "baz", "foo"]);
        assert_eq!(as_strs(f.list("./a").unwrap()), vec!["bar", "baz", "foo"]);
        assert_eq!(as_strs(f.list("a/baz").unwrap()), vec!["pix", "qux"]);
    }

    #[test]
    fn check_real() {
        let mut f = RealDirectoryLister::new();
        assert_eq!(as_strs_real(f.list("example_data/tree").unwrap()), vec!["a", "sub"]);
    }

}
