use std::fmt::{Display, Formatter};
use crate::lang::ast::location::Location;
use crate::lang::ast::Token;
use crate::lang::execute::string;

#[derive(Clone, Debug)]
pub struct TrackedString {
    pub string: String,
    pub location: Location,
}

impl TrackedString {
    pub fn new(string: &str, location: Location) -> TrackedString {
        TrackedString {
            string: string.to_string(),
            location,
        }
    }

    pub fn slice(&self, from: usize, to: usize ) -> TrackedString {
        TrackedString {
            string: self.string[from..to].to_string(),
            location: Location::new(self.location.start + from, self.location.start + from + to)
        }
    }

    pub fn slice_to_end(&self, from: usize) -> TrackedString {
        TrackedString {
            string: self.string[from..].to_string(),
            location: Location::new(self.location.start + from, self.location.end)
        }
    }

    pub fn literal(start: usize, string: &str, end: usize) -> TrackedString {
        TrackedString {
            string: string.to_string(),
            location: Location::new(start, end),
        }
    }

    pub fn prefix(&self, pos: usize) -> TrackedString {
        if !self.location.contains(pos) {
            if self.location.start > pos {
                TrackedString {
                    string: "".to_string(),
                    location: Location::new(self.location.start, self.location.start),
                }
            } else {
                self.clone()
            }
        } else {
            let len = pos - self.location.start;
            TrackedString {
                string: self.string[0..len].to_string(),
                location: Location::new(self.location.start, self.location.start + len),
            }
        }
    }

    pub fn location(&self) -> Location {
        self.location
    }
}

impl Display for TrackedString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.string)
    }
}

impl From<(&str, Location)> for TrackedString {

    fn from(value: (&str, Location)) -> TrackedString {
        TrackedString::new(value.0, value.1)
    }
}

impl From<TrackedString> for String {

    fn from(value: TrackedString) -> String {
        value.string
    }
}

impl From<Token<'_>> for TrackedString {
    fn from(value: Token) -> TrackedString {
        match value {
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
            Token::Equals( l) |
            Token::Declare(l) |
            Token::Separator(_, l) |
            Token::SubStart( l) |
            Token::SubEnd( l) |
            Token::JobStart( l) |
            Token::JobEnd( l) |
            Token::GetItemStart( l) |
            Token::GetItemEnd( l) |
            Token::Pipe( l) |
            Token::Unnamed( l) |
            Token::Named( l) |
            Token::Plus(l) |
            Token::Minus(l) |
            Token::Star(l) |
            Token::Slash(l) |
            Token::Bang(l) |
            Token::ExprModeStart(l) => { TrackedString::new(value.as_string(), l) }
        }
    }
}
