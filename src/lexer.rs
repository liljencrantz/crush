use regex::Regex;
use std::clone::Clone;
use lazy_static::lazy_static;
use crate::lexer::TokenType::{Whitespace, Comment, EOF};

#[derive(Clone)]
#[derive(Debug)]
#[derive(PartialEq)]
pub enum TokenType {
    Pipe,
    Number,
    StringOrWildcard,
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
    static ref lex_data: [(TokenType, Regex); 14] = [
        (TokenType::Separator, Regex::new("^;").unwrap()),
        (TokenType::Pipe, Regex::new(r"^\|").unwrap()),
        (TokenType::Assign, Regex::new(r"^=").unwrap()),
        (TokenType::Equal, Regex::new(r"^==").unwrap()),
        (TokenType::NotEqual, Regex::new(r"^!=").unwrap()),
        (TokenType::Number, Regex::new(r"^-?[0-9]*").unwrap()),
        (TokenType::BlockStart, Regex::new(r"^[`r$*]?\{").unwrap()),
        (TokenType::BlockEnd, Regex::new(r"^\}").unwrap()),
        (TokenType::String, Regex::new(r"^[a-zA-Z][-+_a-z-A-Z0-9]*").unwrap()),
        (TokenType::Wildcard, Regex::new(r"^[a-zA-Z*.?][-+_a-z-A-Z0-9*.?]*").unwrap()),
        (TokenType::Comment, Regex::new("(?m)^#.*$").unwrap()),
        (TokenType::Whitespace, Regex::new(r"^\s*").unwrap()),
        (TokenType::QuotedString, Regex::new(r#"^"([^\\"]|\\.)*""#).unwrap()),
        (TokenType::Error, Regex::new("^.").unwrap()),
    ];
}

impl Lexer {
    fn new(input: &String) -> Lexer {
        return Lexer {
            input: input.clone(),
            idx: 0,
            peeked: None,
        };
    }

    fn next_of_any(&mut self) -> Option<(TokenType, usize, usize)> {
        let mut max_len = 0;
        let mut token_type = Whitespace;
        for (token, re) in lex_data.into_iter() {
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
            return Some((token_type, self.idx - max_len, self.idx));
        }
        return None;
    }

    pub fn peek(&mut self) -> Option<(TokenType, &str)> {
        let tmp = self.next_span();
        self.peeked = tmp;
        return match &self.peeked {
            Some((tt, from, to)) => Some((tt.clone(), &self.input[*from..*to])),
            None => None,
        };
    }

    pub fn peek_type(&mut self) -> TokenType {
        let tmp = self.next_span();
        self.peeked = tmp;
        return match &self.peeked {
            Some((tt, from, to)) => tt.clone(),
            None => EOF,
        };
    }

    fn next_span(&mut self) -> Option<(TokenType, usize, usize)> {
        let s = self.peeked.clone();
        match s {
            None => {
                loop {
                    match self.next_of_any() {
                        Some((token_type, from, to)) => {
                            match token_type {
                                Whitespace | Comment => continue,
                                _ => return Some((token_type, from, to)),
                            }
                        }
                        None => return None,
                    }
                }
            }

            Some(val) => {
                self.peeked = None;
                return Some(val.clone());
            }
        }
    }

    pub fn pop(& mut self) -> Option<(TokenType, &str)> {
        return match &self.next_span() {
            Some((tt, from ,to)) => Some((tt.clone(), &self.input[*from..*to])),
            None => None,
        };
    }
}
/*
pub fn lex_test() {
    let ex = r#"pwd;ls# *{*.txt} ff??.fnurp foo=bar baz = qux snurg
    baz | qux ${mjupp} `{ls} "a" "a|bc ${ggg} `{c} \"d" {pwd}; pwd # tralala "#;
    println!("{}", ex);
    let mut l = Lexer::new(&String::from(ex));
    loop {
        match l.pop() {
            None => {break;},
            Some((t, s)) => {
                println!("WEEE {:?} '{}'", t, s);
            },
        }
    }
}
*/
