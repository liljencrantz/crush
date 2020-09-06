use crate::lang::data::scope::Scope;
use crate::lang::errors::{CrushResult, error};
use crate::lang::parser::tokenize;
use std::collections::HashMap;
use ordered_map::OrderedMap;
use crate::lang::ast::{TokenNode, TokenType};
use crate::lang::argument::ArgumentDefinition;

pub struct Completion {
    completion: String,
    position: usize,
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
        println!("Compare {} and {}", name, &arg.data);
        if name.starts_with(&arg.data) {
            res.push(Completion {
                completion: name.strip_prefix(&arg.data).unwrap().to_string(),
                position: arg.end,
            })
        }
    }

    Ok(res)
}

pub fn complete_parse(line: &str, cursor: usize) -> CrushResult<(Option<String>, Vec<ArgumentDefinition>, Option<TokenNode>)> {
    let tokens = tokenize(line)?;

    let mut cmd = None;
    let mut args = Vec::new();
    let mut token = None;

    for t in tokens.iter() {
        if t.start < cursor && t.end >= cursor {
            token = Some(t.clone());
            break;
        }
    }

    Ok((cmd, args, token))
}

pub fn complete(line: &str, cursor: usize, scope: Scope) -> CrushResult<Vec<Completion>> {
    let (cmd, args, token) = complete_parse(line, cursor)?;

    match token {
        None => Ok(Vec::new()),
        Some(tok) => complete_cmd(cmd, args, tok, scope),
    }
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
