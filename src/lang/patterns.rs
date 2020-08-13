use crate::lang::value::Value;
use crate::util::glob::Glob;
use regex::Regex;

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
