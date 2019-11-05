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
pub enum CellTypeToken {
    Name,
    Begin,
    End,
    EOF,
    Error,
}

pub type CellTypeLexer = BaseLexer<CellTypeToken>;

impl CellTypeLexer {
    pub fn new(input: &String) -> CellTypeLexer {
        return BaseLexer::construct(
            input,
            &LEX_DATA,
            CellTypeToken::Error,
            CellTypeToken::EOF,
            &IGNORED,
        );
    }
}

lazy_static! {
static ref IGNORED: HashSet<CellTypeToken> = {
let mut ignored = HashSet::new();
ignored
};
}

lazy_static! {
    static ref LEX_DATA: Vec<(CellTypeToken, Regex)> = vec![
        (CellTypeToken::Begin, Regex::new("^<").unwrap()),
        (CellTypeToken::End, Regex::new("^>").unwrap()),
        (CellTypeToken::Name, Regex::new("^[a-z]*").unwrap()),
        (CellTypeToken::Error, Regex::new("^.").unwrap()),
    ];
}


#[cfg(test)]
mod tests {
    use super::*;
    use super::CellTypeToken::*;

    fn tokens(lexer: &mut CellTypeLexer) -> Vec<CellTypeToken> {
        let mut res: Vec<CellTypeToken> = Vec::new();
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
        let mut l = CellTypeLexer::new(&String::from("list<list<integer>>"));
        let tt = tokens(&mut l);
        assert_eq!(tt, vec![Name, Begin, Name, Begin, Name, End, End, EOF]);
    }

}
