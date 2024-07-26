use std::fmt::{Display, Formatter};
use crate::lang::ast::lexer::Spanned;
use crate::lang::ast::location::Location;
use crate::lang::ast::tracked_string::TrackedString;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Token<'input> {
    LogicalOperator(&'input str, Location),
    UnaryOperator(&'input str, Location),
    ComparisonOperator(&'input str, Location),
    Bang(Location),
    Plus(Location),
    Minus(Location),
    Star(Location),
    Slash(Location),
    QuotedString(&'input str, Location),
    StringOrGlob(&'input str, Location),
    Identifier(&'input str, Location),
    Flag(&'input str, Location),
    QuotedFile(&'input str, Location),
    FileOrGlob(&'input str, Location),
    Regex(&'input str, Location),
    Integer(&'input str, Location),
    Float(&'input str, Location),
    MemberOperator(Location),
    Equals(Location),
    Declare(Location),
    Separator(&'input str, Location),
    SubStart(Location),
    SubEnd(Location),
    JobStart(Location),
    JobEnd(Location),
    GetItemStart(Location),
    GetItemEnd(Location),
    Pipe(Location),
    Unnamed(Location),
    Named(Location),
    ExprModeStart(Location),
}

impl Token<'_> {
    pub fn location(&self) -> Location {
        match self {
            Token::LogicalOperator(_, l) |
            Token::UnaryOperator(_, l) |
            Token::ComparisonOperator(_, l) |
            Token::QuotedString(_, l) |
            Token::StringOrGlob(_, l) |
            Token::Identifier(_, l) |
            Token::Flag(_, l) |
            Token::QuotedFile(_, l) |
            Token::FileOrGlob(_, l) |
            Token::Regex(_, l) |
            Token::Integer(_, l) |
            Token::Float(_, l) |
            Token::MemberOperator(l) |
            Token::Equals(l) |
            Token::Declare(l) |
            Token::Separator(_, l) |
            Token::SubStart(l) |
            Token::SubEnd(l) |
            Token::JobStart(l) |
            Token::JobEnd(l) |
            Token::GetItemStart(l) |
            Token::GetItemEnd(l) |
            Token::Pipe(l) |
            Token::Unnamed(l) |
            Token::Named(l) |
            Token::Bang(l) |
            Token::Plus(l) |
            Token::Minus(l) |
            Token::Star(l) |
            Token::Slash(l) |
            Token::ExprModeStart(l) => *l,
        }
    }

    pub fn as_string(&self) -> &str {
        match self {
            Token::LogicalOperator(s, _) |
            Token::UnaryOperator(s, _) |
            Token::ComparisonOperator(s, _) |
            Token::QuotedString(s, _) |
            Token::StringOrGlob(s, _) |
            Token::Identifier(s, _) |
            Token::Flag(s, _) |
            Token::QuotedFile(s, _) |
            Token::FileOrGlob(s, _) |
            Token::Regex(s, _) |
            Token::Integer(s, _) |
            Token::Separator(s, _) |
            Token::Float(s, _) => s,
            Token::MemberOperator(_) => ":",
            Token::Equals(_) => "=",
            Token::Declare(_) => ":=",
            Token::SubStart(_) => "(",
            Token::SubEnd(_) => "_",
            Token::JobStart(_) => "{",
            Token::JobEnd(_) => "}",
            Token::GetItemStart(_) => "[",
            Token::GetItemEnd(_) => "]",
            Token::Pipe(_) => "|",
            Token::Unnamed(_) => "@",
            Token::Named(_) => "@@",
            Token::ExprModeStart(_) => "m(",
            Token::Bang(_) => "!",
            Token::Plus(_) => "+",
            Token::Minus(_) => "-",
            Token::Star(_) => "*",
            Token::Slash(_) => "/",
        }
    }
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&TrackedString::from(self.clone()).string)
    }
}
impl From<Token<'_>> for String {
    fn from(token: Token) -> String {
        TrackedString::from(token).string
    }
}
impl<'a> Into<Spanned<'a>> for Token<'a> {
    fn into(self) -> Spanned<'a> {
        let loc = match &self {
            Token::LogicalOperator(_, l) |
            Token::UnaryOperator(_, l) |
            Token::QuotedString(_, l) |
            Token::StringOrGlob(_, l) |
            Token::Identifier(_, l) |
            Token::Flag(_, l) |
            Token::QuotedFile(_, l) |
            Token::FileOrGlob(_, l) |
            Token::Regex(_, l) |
            Token::Integer(_, l) |
            Token::ComparisonOperator(_, l) |
            Token::Float(_, l) |
            Token::MemberOperator(l) |
            Token::Equals(l) |
            Token::Declare(l) |
            Token::Separator(_, l) |
            Token::SubStart(l) |
            Token::SubEnd(l) |
            Token::JobStart(l) |
            Token::JobEnd(l) |
            Token::GetItemStart(l) |
            Token::GetItemEnd(l) |
            Token::Pipe(l) |
            Token::Unnamed(l) |
            Token::Named(l) |
            Token::Bang(l) |
            Token::Plus(l) |
            Token::Minus(l) |
            Token::Star(l) |
            Token::Slash(l) |
            Token::ExprModeStart(l) => { l }
        };
        Ok((loc.start, self, loc.end))
    }
}
