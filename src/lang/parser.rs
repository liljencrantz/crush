use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::job::Job;
use crate::lang::data::scope::Scope;
use crate::lang::ast::{TokenNode, JobListNode};

lalrpop_mod!(pub lalrparser, "/lang/lalrparser.rs");

pub fn parse_name(s: &str) -> Option<Vec<String>> {
    let res = s.split('/').collect::<Vec<&str>>();
    for i in res.iter() {
        if i.is_empty() {
            return None;
        }
    }
    Some(res.iter().map(|e| e.to_string()).collect())
}

pub fn parse(s: &str, env: &Scope) -> CrushResult<Vec<Job>> {
    to_crush_error(lalrparser::JobListParser::new().parse(s))?.generate(env)
}

pub fn ast(s: &str) -> CrushResult<JobListNode> {
    to_crush_error(lalrparser::JobListParser::new().parse(s))
}

pub fn tokenize(s: &str) -> CrushResult<Vec<TokenNode>>{
    Ok(to_crush_error(lalrparser::TokenListParser::new().parse(s))?.tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_token_offsets() {
        let tok = tokenize("123:123.4 foo=\"bar\"").unwrap();
        assert_eq!(tok.len(), 6);
        assert_eq!(tok[0].location(), (0usize,3usize));
        assert_eq!(tok[1].location(), (3usize,4usize));
        assert_eq!(tok[2].location(), (4usize,9usize));
        assert_eq!(tok[3].location(), (10usize,13usize));
        assert_eq!(tok[4].location(), (13usize,14usize));
        assert_eq!(tok[5].location(), (14usize,19usize));
    }

    #[test]
    fn check_token_newline() {
        let tok = tokenize("123# comment\nggg").unwrap();
        assert_eq!(tok.len(), 3);
        assert_eq!(tok[0].location(), (0usize,3usize));
        assert_eq!(tok[1].location(), (12usize,13usize));
        assert_eq!(tok[2].location(), (13usize,16usize));
    }
}
