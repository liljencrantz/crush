use super::lexer::Spanned;
use super::location::Location;
use super::tracked_string::TrackedString;
use std::fmt::{Display, Formatter};

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
    Comment(&'input str, Location),
    Identifier(&'input str, Location),
    Flag(&'input str, Location),
    QuotedFile(&'input str, Location),
    Glob(&'input str, Location),
    File(&'input str, Location),
    String(&'input str, Location),
    Regex(&'input str, Location),
    Integer(&'input str, Location),
    Float(&'input str, Location),
    MemberOperator(Location),
    Equals(Location),
    Declare(Location),
    Separator(&'input str, Location),
    SubStart(Location),
    SubEnd(Location),
    BlockStart(Location),
    BlockEnd(Location),
    GetItemStart(Location),
    GetItemEnd(Location),
    Pipe(Location),
    Unnamed(Location),
    Named(Location),
    ExprModeStart(Location),
    For(Location),
    While(Location),
    Loop(Location),
    If(Location),
    Else(Location),
    Return(Location),
    Break(Location),
    Continue(Location),
    Background(Location),
}

impl Token<'_> {
    pub fn location(&self) -> Location {
        match self {
            Token::LogicalOperator(_, l)
            | Token::UnaryOperator(_, l)
            | Token::ComparisonOperator(_, l)
            | Token::QuotedString(_, l)
            | Token::String(_, l)
            | Token::Comment(_, l)
            | Token::File(_, l)
            | Token::Glob(_, l)
            | Token::Identifier(_, l)
            | Token::Flag(_, l)
            | Token::QuotedFile(_, l)
            | Token::Regex(_, l)
            | Token::Integer(_, l)
            | Token::Float(_, l)
            | Token::MemberOperator(l)
            | Token::Equals(l)
            | Token::Declare(l)
            | Token::Separator(_, l)
            | Token::SubStart(l)
            | Token::SubEnd(l)
            | Token::BlockStart(l)
            | Token::BlockEnd(l)
            | Token::GetItemStart(l)
            | Token::GetItemEnd(l)
            | Token::Pipe(l)
            | Token::Unnamed(l)
            | Token::Named(l)
            | Token::Bang(l)
            | Token::Plus(l)
            | Token::Minus(l)
            | Token::Star(l)
            | Token::Slash(l)
            | Token::For(l)
            | Token::While(l)
            | Token::Loop(l)
            | Token::If(l)
            | Token::Else(l)
            | Token::Return(l)
            | Token::Break(l)
            | Token::Continue(l)
            | Token::ExprModeStart(l)
            | Token::Background(l) => *l,
        }
    }

    pub fn as_string(&self) -> &str {
        match self {
            Token::LogicalOperator(s, _)
            | Token::UnaryOperator(s, _)
            | Token::ComparisonOperator(s, _)
            | Token::QuotedString(s, _)
            | Token::String(s, _)
            | Token::Comment(s, _)
            | Token::File(s, _)
            | Token::Glob(s, _)
            | Token::Identifier(s, _)
            | Token::Flag(s, _)
            | Token::QuotedFile(s, _)
            | Token::Regex(s, _)
            | Token::Integer(s, _)
            | Token::Separator(s, _)
            | Token::Float(s, _) => s,
            Token::MemberOperator(_) => ":",
            Token::Equals(_) => "=",
            Token::Declare(_) => ":=",
            Token::SubStart(_) => "(",
            Token::SubEnd(_) => "_",
            Token::BlockStart(_) => "{",
            Token::BlockEnd(_) => "}",
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
            Token::For(_) => "for",
            Token::While(_) => "while",
            Token::Loop(_) => "loop",
            Token::If(_) => "if",
            Token::Else(_) => "else",
            Token::Return(_) => "return",
            Token::Break(_) => "break",
            Token::Continue(_) => "continue",
            Token::Background(_) => "&",       
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
            Token::LogicalOperator(_, l)
            | Token::UnaryOperator(_, l)
            | Token::QuotedString(_, l)
            | Token::String(_, l)
            | Token::Comment(_, l)
            | Token::File(_, l)
            | Token::Glob(_, l)
            | Token::Identifier(_, l)
            | Token::Flag(_, l)
            | Token::QuotedFile(_, l)
            | Token::Regex(_, l)
            | Token::Integer(_, l)
            | Token::ComparisonOperator(_, l)
            | Token::Float(_, l)
            | Token::MemberOperator(l)
            | Token::Equals(l)
            | Token::Declare(l)
            | Token::Separator(_, l)
            | Token::SubStart(l)
            | Token::SubEnd(l)
            | Token::BlockStart(l)
            | Token::BlockEnd(l)
            | Token::GetItemStart(l)
            | Token::GetItemEnd(l)
            | Token::Pipe(l)
            | Token::Unnamed(l)
            | Token::Named(l)
            | Token::Bang(l)
            | Token::Plus(l)
            | Token::Minus(l)
            | Token::Star(l)
            | Token::Slash(l)
            | Token::For(l)
            | Token::While(l)
            | Token::Loop(l)
            | Token::If(l)
            | Token::Else(l)
            | Token::Return(l)
            | Token::Break(l)
            | Token::Continue(l)
            | Token::ExprModeStart(l)
            | Token::Background(l) => l,
        };
        Ok((loc.start, self, loc.end))
    }
}
