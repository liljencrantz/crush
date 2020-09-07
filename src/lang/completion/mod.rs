use crate::lang::data::scope::Scope;
use crate::lang::errors::{CrushResult, error};
use crate::lang::parser::tokenize;
use std::collections::HashMap;
use ordered_map::OrderedMap;
use crate::lang::ast::{TokenNode, TokenType};
use crate::lang::argument::ArgumentDefinition;
use crate::lang::value::ValueDefinition;

pub struct Completion {
    completion: String,
    position: usize,
}

struct ParseState {
    vec: Vec<TokenNode>,
    idx: usize,
}


struct ParseResult {
    cmd: Option<String>,
    previous_arguments: Vec<ArgumentDefinition>,
    partial_argument: Option<ArgumentDefinition>,
}

impl ParseState {
    pub fn new(vec: Vec<TokenNode>) -> ParseState {
        ParseState {
            vec,
            idx: 0,
        }
    }

    pub fn next(&mut self) -> Option<&str> {
        self.idx += 1;
        self.vec.get(self.idx).map(|t| t.data.as_str())
    }

    pub fn peek(&self) -> Option<&str> {
        self.vec.get(self.idx + 1).map(|t| t.data.as_str())
    }

    pub fn location(&self) -> Option<(usize, usize)> {
        self.vec.get(self.idx).map(|t| (t.start, t.end))
    }
}

impl Completion {
    pub fn complete(&self,
                    line: &str,
    ) -> String {
        let mut res = line.to_string();
        res.insert_str(self.position, &self.completion);
        res
    }
}

fn complete_cmd(cmd: Option<String>, args: Vec<ArgumentDefinition>, arg: TokenNode, scope: Scope) -> CrushResult<Vec<Completion>> {
    let mut map = OrderedMap::new();
    scope.dump(&mut map)?;

    let mut res = Vec::new();

    for name in map.keys() {
        if name.starts_with(&arg.data) {
            res.push(Completion {
                completion: name.strip_prefix(&arg.data).unwrap().to_string(),
                position: arg.end,
            })
        }
    }

    Ok(res)
}

fn complete_parse(line: &str, cursor: usize) -> CrushResult<ParseResult> {
    error("Not implemented")
}

pub fn complete(line: &str, cursor: usize, scope: Scope) -> CrushResult<Vec<Completion>> {
    let parse_result = complete_parse(line, cursor)?;
/*
    match token {
        None => Ok(Vec::new()),
        Some(tok) => complete_cmd(parse_result, scope),
    }*/
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::value::Value;

    #[test]
    fn check_simple_test() {
        let line = "ab";
        let cursor = 2;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, s).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd");
    }

    #[test]
    fn check_cursor_in_middle_of_token() {
        let line = "ab";
        let cursor = 1;

        let s = Scope::create_root();
        s.declare("abcd", Value::Empty()).unwrap();
        let completions = complete(line, cursor, s).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "abcd");
    }

    #[test]
    fn check_multiple_token() {
        let line = "ab cd ef";
        let cursor = 5;

        let s = Scope::create_root();
        s.declare("cdef", Value::Empty()).unwrap();
        let completions = complete(line, cursor, s).unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(&completions[0].complete(line), "ab cdef ef");
    }
}
