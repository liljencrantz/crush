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
pub enum ValueTypeToken {
    Name,
    Begin,
    End,
    Sep,
    To,
    Whitespace,
    EOF,
    Error,
}

pub type ValueTypeLexer = BaseLexer<ValueTypeToken>;

impl ValueTypeLexer {
    pub fn new(input: &str) -> ValueTypeLexer {
        return BaseLexer::construct(
            input,
            &LEX_DATA,
            ValueTypeToken::Error,
            ValueTypeToken::EOF,
            &IGNORED,
        );
    }
}

lazy_static! {
    static ref IGNORED: HashSet<ValueTypeToken> = {
        let mut ignored = HashSet::new();
        ignored.insert(ValueTypeToken::Whitespace);
        ignored
    };
}

lazy_static! {
    static ref LEX_DATA: Vec<(ValueTypeToken, Regex)> = vec![
        (ValueTypeToken::Begin, Regex::new("^<").unwrap()),
        (ValueTypeToken::End, Regex::new("^>").unwrap()),
        (ValueTypeToken::Sep, Regex::new("^,").unwrap()),
        (ValueTypeToken::To, Regex::new("^:").unwrap()),
        (ValueTypeToken::Whitespace, Regex::new("^ *").unwrap()),
        (ValueTypeToken::Name, Regex::new("^[a-z]*").unwrap()),
        (ValueTypeToken::Error, Regex::new("^.").unwrap()),
    ];
}


#[cfg(test)]
mod tests {
    use super::*;
    use super::ValueTypeToken::*;

    fn tokens(lexer: &mut ValueTypeLexer) -> Vec<ValueTypeToken> {
        let mut res: Vec<ValueTypeToken> = Vec::new();
        loop {
            let t = lexer.pop().0;
            res.push(t);
            if t == EOF || t == Error {
                break;
            }
        }
        return res;
    }

    #[test]
    fn blocks() {
        let mut l = ValueTypeLexer::new(&String::from("list<   :  dict<integer,bool>><::>"));
        let tt = tokens(&mut l);
        assert_eq!(tt, vec![Name, Begin, To, Name, Begin, Name, Sep, Name, End, End, Begin, To, To, End, EOF]);
    }

    #[test]
    fn error() {
        let mut l = ValueTypeLexer::new(&String::from("7"));
        let tt = tokens(&mut l);
        assert_eq!(tt, vec![Error]);
    }
}
