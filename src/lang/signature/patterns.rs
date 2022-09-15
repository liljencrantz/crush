use crate::lang::value::Value;
use crate::util::glob::Glob;
use regex::Regex;
use std::fmt::{Display, Formatter};

/**
  A type representing a set of text patterns. It accepts any number of strings (exact match) as well
  as globs and regexes. The test method can be used to check if the patterns match.
*/

pub struct Patterns {
    patterns: Vec<Value>,
}

impl Patterns {
    pub fn new() -> Patterns {
        Patterns {
            patterns: Vec::new(),
        }
    }

    pub fn expand_string(&mut self, string: String) {
        self.patterns.push(Value::String(string));
    }

    pub fn expand_glob(&mut self, glob: Glob) {
        self.patterns.push(Value::Glob(glob));
    }

    pub fn expand_regex(&mut self, def: String, re: Regex) {
        self.patterns.push(Value::Regex(def, re));
    }

    pub fn test(&self, value: &str) -> bool {
        for v in &self.patterns {
            if v.matches(value).unwrap() {
                return true;
            }
        }
        false
    }
}

impl Display for Patterns {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("[")?;
        f.write_str(
            &self
                .patterns
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        )?;
        f.write_str("]")?;
        Ok(())
    }
}
