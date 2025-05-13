use std::path::Path;
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
}
