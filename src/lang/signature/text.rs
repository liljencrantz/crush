use std::path::{Path, PathBuf};
use std::sync::Arc;

/**
A type representing a value with a textual representation. It is used in the signature of builtin commands that
accept any type of text value as arguments, e.g. the string matching functions in globs and regexes.
*/
pub enum Text {
    File(Arc<Path>),
    String(Arc<str>),
}

impl Text {
    pub fn as_string(&self) -> String {
        match self {
            Text::File(p) => p.to_str().unwrap_or("").to_string(),
            Text::String(s) => s.to_string(),
        }
    }

    pub fn as_path(&self) -> PathBuf {
        match self {
            Text::File(p) => p.to_path_buf(),
            Text::String(s) => PathBuf::from(s.to_string()),
        }
    }
}
