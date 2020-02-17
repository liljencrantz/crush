use regex::Regex;
use std::clone::Clone;
use lazy_static::lazy_static;
use std::collections::HashSet;
use std::hash::Hash;
use crate::base_lexer::BaseLexer;


#[derive(Clone)]
#[derive(Copy)]
#[derive(Debug)]
#[derive(PartialEq, Eq, Hash)]
pub enum TokenType {
    Pipe,
    Integer,
    Float,
    String,
    Glob,
    ModeStart,
    ModeEnd,
    SubscriptStart,
    SubscriptEnd,
    Comment,
    Whitespace,
    QuotedString,
    Assign,
    Separator,
    Error,
    Match,
    NotMatch,
    Variable,
    Field,
    Regex,
    EOF,
}

pub type Lexer = BaseLexer<TokenType>;

impl Lexer {
    pub fn new(input: &str, ) -> Lexer {
        return BaseLexer::construct(
            input,
            &LEX_DATA,
            TokenType::Error,
            TokenType::EOF,
            &IGNORED,
        );
    }

}

lazy_static! {
static ref IGNORED: HashSet<TokenType> = {
let mut ignored = HashSet::new();
ignored.insert(TokenType::Whitespace);
ignored.insert(TokenType::Comment);
ignored
};
}

lazy_static! {
    static ref LEX_DATA: Vec<(TokenType, Regex)> = vec![
        (TokenType::Separator, Regex::new("^;").unwrap()),
        (TokenType::Separator, Regex::new("^\n").unwrap()),
        (TokenType::Pipe, Regex::new(r"^\|").unwrap()),

        (TokenType::Assign, Regex::new(r"^=").unwrap()),

        (TokenType::Match, Regex::new(r"^=~").unwrap()),
        (TokenType::NotMatch, Regex::new(r"^!~").unwrap()),

        (TokenType::Float, Regex::new(r"^[0-9]+\.[0-9]+").unwrap()),
        (TokenType::Integer, Regex::new(r"^[0-9]+").unwrap()),

        (TokenType::Variable, Regex::new(r"^\$[a-zA-Z_][\.a-zA-Z_0-9]*").unwrap()),
        (TokenType::Field, Regex::new(r"^%[a-zA-Z_][\.a-zA-Z_0-9]*").unwrap()),

        (TokenType::ModeStart, Regex::new(r"^([`*]?|[a-z]+)\{").unwrap()),
        (TokenType::ModeEnd, Regex::new(r"^\}").unwrap()),
        (TokenType::ModeStart, Regex::new(r"^\(").unwrap()),
        (TokenType::ModeEnd, Regex::new(r"^\)").unwrap()),

        (TokenType::SubscriptStart, Regex::new(r"^\[").unwrap()),
        (TokenType::SubscriptEnd, Regex::new(r"^]").unwrap()),

        (TokenType::Regex, Regex::new(r"^regex\{([^}\\]|\\.)+\}").unwrap()),

        (TokenType::String, Regex::new(r"^[/._a-zA-Z][/.:_a-z-A-Z0-9]*").unwrap()),
        (TokenType::Glob, Regex::new(r"^[/._a-zA-Z*.?][/_a-z-A-Z0-9*.?]*").unwrap()),
        (TokenType::Comment, Regex::new("^#[^\n]*").unwrap()),
        (TokenType::Whitespace, Regex::new(r"^[ \t]+").unwrap()),
        (TokenType::QuotedString, Regex::new(r#"^"([^\\"]|\\.)*""#).unwrap()),
        (TokenType::Error, Regex::new("^.").unwrap()),
    ];
}

pub fn extract_tokens(lexer: &mut Lexer) -> Vec<TokenType> {
    let mut res: Vec<TokenType> = Vec::new();
    loop {
        let t = lexer.pop().0;
        res.push(t);
        if t == TokenType::EOF || t == TokenType::Error {
            break;
        }
    }
    return res;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks() {
        let mut l = Lexer::new(&String::from("echo `{foo}"));
        let tt = extract_tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::String, TokenType::ModeStart, TokenType::String, TokenType::ModeEnd,
            TokenType::EOF]);
    }

    #[test]
    fn globs() {
        let mut l = Lexer::new(&String::from("echo foo.* abc??def"));
        let tt = extract_tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::String, TokenType::Glob, TokenType::Glob, TokenType::EOF]);
    }

    #[test]
    fn list() {
        let mut l = Lexer::new(&String::from("[a]"));
        let tt = extract_tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::SubscriptStart, TokenType::String, TokenType::SubscriptEnd, TokenType::EOF]);
    }

    #[test]
    fn separators() {
        let mut l = Lexer::new(&String::from("a|b;c\nd\n"));
        let tt = extract_tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::String, TokenType::Pipe, TokenType::String, TokenType::Separator,
            TokenType::String, TokenType::Separator,
            TokenType::String, TokenType::Separator,
            TokenType::EOF]);
    }

    #[test]
    fn comments() {
        let mut l = Lexer::new(&String::from("a # this is a comment\n"));
        let tt = extract_tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::String, TokenType::Separator, TokenType::EOF]);
    }

    #[test]
    fn numbers() {
        let mut l = Lexer::new(&String::from("b 2 d 1.5"));
        let tt = extract_tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::String, TokenType::Integer, TokenType::String, TokenType::Float, TokenType::EOF]);
    }

    #[test]
    fn quoted_string() {
        let mut l = Lexer::new(&String::from(r##" "abc"  "\" ggg" "\d \\"  "##));
        let tt = extract_tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::QuotedString, TokenType::QuotedString,
            TokenType::QuotedString, TokenType::EOF]);
    }

    #[test]
    fn assign() {
        let mut l = Lexer::new(&String::from("foo=bar baz = 7"));
        let tt = extract_tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::String, TokenType::Assign, TokenType::String,
            TokenType::String, TokenType::Assign, TokenType::Integer,
            TokenType::EOF,
        ]);
    }

    #[test]
    fn variables_and_fields() {
        let mut l = Lexer::new(&String::from("$foo %bar $foo.bar %baz.qux"));
        let tt = extract_tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::Variable, TokenType::Field, TokenType::Variable, TokenType::Field, TokenType::EOF]);
    }

    #[test]
    fn regex() {
        let mut l = Lexer::new(&String::from(r"   regex{^.$}   regex{{foo\}}  "));
        let tt = extract_tokens(&mut l);
        assert_eq!(tt, vec![
            TokenType::Regex, TokenType::Regex, TokenType::EOF]);
    }
}
