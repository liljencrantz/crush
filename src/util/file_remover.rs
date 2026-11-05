/**
A simple wrapper around std::fs::{remove_file, remove_dir} to allow for unit testing via fakes,
the same pattern directory_lister.rs uses for listing directories.

There are two implementations, the real one can be instantiated via the file_remover() function.
There is also a fake remover for tests, accessible via FakeFileRemover::new().
*/
use crate::lang::errors::CrushResult;
use std::path::Path;

pub trait FileRemover {
    fn remove_file(&self, path: &Path) -> CrushResult<()>;
    fn remove_dir(&self, path: &Path) -> CrushResult<()>;
}

pub struct RealFileRemover {}

pub fn file_remover() -> RealFileRemover {
    RealFileRemover {}
}

impl FileRemover for RealFileRemover {
    fn remove_file(&self, path: &Path) -> CrushResult<()> {
        Ok(std::fs::remove_file(path)?)
    }

    fn remove_dir(&self, path: &Path) -> CrushResult<()> {
        Ok(std::fs::remove_dir(path)?)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// Records every path it was asked to remove, in order, instead of touching the real
    /// filesystem.
    pub struct FakeFileRemover {
        removed: Mutex<Vec<PathBuf>>,
    }

    impl FakeFileRemover {
        pub fn new() -> FakeFileRemover {
            FakeFileRemover {
                removed: Mutex::new(Vec::new()),
            }
        }

        pub fn removed(&self) -> Vec<PathBuf> {
            self.removed.lock().unwrap().clone()
        }
    }

    impl FileRemover for FakeFileRemover {
        fn remove_file(&self, path: &Path) -> CrushResult<()> {
            self.removed.lock().unwrap().push(path.to_path_buf());
            Ok(())
        }

        fn remove_dir(&self, path: &Path) -> CrushResult<()> {
            self.removed.lock().unwrap().push(path.to_path_buf());
            Ok(())
        }
    }
}
