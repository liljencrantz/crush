use regex::Regex;
use std::clone::Clone;
use lazy_static::lazy_static;
use crate::lexer::TokenType::{Whitespace, Comment, EOF};

#[derive(Clone)]
#[derive(Copy)]
#[derive(Debug)]
#[derive(PartialEq)]
pub enum TokenType {
    Pipe,
    Integer,
    String,
    Glob,
    BlockStart,
    BlockEnd,
    Comment,
    Whitespace,
    QuotedString,
    WildcardOne,
    WildcardMany,
    Assign,
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Separator,
    Error,
    EOF,
}

pub struct Lexer {
    input: String,
    idx: usize,
    peeked: Option<(TokenType, usize, usize)>,
}

lazy_static! {
    static ref LEX_DATA: [(TokenType, Regex); 18] = [
        (TokenType::Separator, Regex::new("^;").unwrap()),
        (TokenType::Pipe, Regex::new(r"^\|").unwrap()),

        (TokenType::Assign, Regex::new(r"^=").unwrap()),
        (TokenType::Equal, Regex::new(r"^==").unwrap()),
        (TokenType::LessThan, Regex::new(r"^<").unwrap()),
        (TokenType::LessThanOrEqual, Regex::new(r"^<=").unwrap()),
        (TokenType::GreaterThan, Regex::new(r"^>").unwrap()),
        (TokenType::GreaterThanOrEqual, Regex::new(r"^>=").unwrap()),
        (TokenType::NotEqual, Regex::new(r"^!=").unwrap()),

        (TokenType::Integer, Regex::new(r"^[0-9]+").unwrap()),

        (TokenType::BlockStart, Regex::new(r"^[`r$*%]?\{").unwrap()),
        (TokenType::BlockEnd, Regex::new(r"^\}").unwrap()),
        (TokenType::String, Regex::new(r"^[-.+_a-zA-Z][-.+_a-z-A-Z0-9]*").unwrap()),
        (TokenType::Glob, Regex::new(r"^[-.+_a-zA-Z*.?][-+_a-z-A-Z0-9*.?]*").unwrap()),
        (TokenType::Comment, Regex::new("(?m)^#.*$").unwrap()),
        (TokenType::Whitespace, Regex::new(r"^\s+").unwrap()),
        (TokenType::QuotedString, Regex::new(r#"^"([^\\"]|\\.)*""#).unwrap()),
        (TokenType::Error, Regex::new("^.").unwrap()),
    ];
}

impl Lexer {
    pub fn new(input: &String) -> Lexer {
        return Lexer {
            input: input.clone(),
            idx: 0,
            peeked: None,
        };
    }

    fn next_of_any(&mut self) -> (TokenType, usize, usize) {
        let mut max_len = 0;
        let mut token_type = Whitespace;
        if self.idx >= self.input.len() {
            return (EOF, 0, 0);
        }
        for (token, re) in LEX_DATA.into_iter() {
            //let re = Regex::new(r".");
            match re.find(&self.input[self.idx..]) {
                Some(mat) => {
                    if mat.end() > max_len {
                        max_len = mat.end();
                        token_type = token.clone();
                    }
                }
                None => {}
            }
        }
        if max_len > 0 {
            self.idx += max_len;
            return (token_type, self.idx - max_len, self.idx);
        }
        return (TokenType::Error, 0, 0);
    }

    pub fn peek(&mut self) -> (TokenType, &str) {
        let (tt, from, to) = self.next_span();
        self.peeked = Some((tt, from, to));
        return (tt, &self.input[from..to]);
    }

    fn next_span(&mut self) -> (TokenType, usize, usize) {
        let s = self.peeked;
        match s {
            None => {
                loop {
                    let (token_type, from, to) = self.next_of_any();
                    match token_type {
                        Whitespace | Comment => continue,
                        _ => return (token_type, from, to),
                    }
                }
            }

            Some(val) => {
                self.peeked = None;
                return val;
            }
        }
    }

    pub fn pop(&mut self) -> (TokenType, &str) {
        let (tt, from, to) = self.next_span();
        return (tt, &self.input[from..to]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(lexer: &mut Lexer) -> Vec<TokenType> {
        let mut res: Vec<TokenType> = Vec::new();
        loop {
            let t = lexer.pop().0;
            res.push(t);
            if (t == TokenType::EOF || t == TokenType::Error) {
                break;
            }
        }
        return res;
    }

    #[test]
    fn blocks() {
        let mut l = Lexer::new(&String::from("echo %{foo}"));
        let tt = tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::String, TokenType::BlockStart, TokenType::String, TokenType::BlockEnd,
            TokenType::EOF]);
    }

    #[test]
    fn globs() {
        let mut l = Lexer::new(&String::from("echo foo.* abc??def"));
        let tt = tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::String, TokenType::Glob, TokenType::Glob, TokenType::EOF]);
    }

    #[test]
    fn separators() {
        let mut l = Lexer::new(&String::from("a|b;c"));
        let tt = tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::String, TokenType::Pipe, TokenType::String, TokenType::Separator, TokenType::String, TokenType::EOF]);
    }

    #[test]
    fn comments() {
        let mut l = Lexer::new(&String::from("a # this is a comment"));
        let tt = tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::String, TokenType::EOF]);
    }

    #[test]
    fn numbers() {
        let mut l = Lexer::new(&String::from("b 2 d"));
        let tt = tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::String, TokenType::Integer, TokenType::String, TokenType::EOF]);
    }

    #[test]
    fn quoted_string() {
        let mut l = Lexer::new(&String::from(r##" "abc"  "\" ggg" "\d \\"  "##));
        let tt = tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::QuotedString, TokenType::QuotedString,
            TokenType::QuotedString, TokenType::EOF]);
    }

    #[test]
    fn assign() {
        let mut l = Lexer::new(&String::from("foo=bar baz = 7"));
        let tt = tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::String, TokenType::Assign, TokenType::String,
            TokenType::String, TokenType::Assign, TokenType::Integer,
            TokenType::EOF,
        ]);
    }

    #[test]
    fn comparison_operators() {
        let mut l = Lexer::new(&String::from("== >= > < <= !="));
        let tt = tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::Equal, TokenType::GreaterThanOrEqual, TokenType::GreaterThan,
            TokenType::LessThan, TokenType::LessThanOrEqual, TokenType::NotEqual, TokenType::EOF]);
    }
}
