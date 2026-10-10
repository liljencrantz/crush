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
        self.patterns.push(Value::from(string));
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

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Like `test()`, but a plain string pattern means "appears somewhere in `haystack`"
    /// rather than "equals `haystack` exactly" -- the semantics `vt:wait_for` needs (did
    /// this text ever get printed on a screen that also has lots of *other* text on it),
    /// as opposed to `test()`'s exact-match semantics (e.g. "is this one service name").
    /// A glob or regex pattern already supports "appears somewhere" on its own (an
    /// unanchored regex, or a glob the caller wrote with explicit `*...*` wildcards), so
    /// only the plain-string case needs different handling here.
    pub fn test_contains(&self, haystack: &str) -> bool {
        for v in &self.patterns {
            let matched = match v {
                Value::String(s) => haystack.contains(&s.to_string()),
                Value::Glob(g) => g.matches(haystack),
                Value::Regex(_, re) => re.is_match(haystack),
                _ => false,
            };
            if matched {
                return true;
            }
        }
        false
    }
}

impl Display for Patterns {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        if self.patterns.len() == 1 {
            self.patterns[0].fmt(f)
        } else {
            f.write_str("[")?;
            f.write_str(
                &self
                    .patterns
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            )?;
            f.write_str("]")
        }
    }
}
