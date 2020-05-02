use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::job::Job;
use crate::lang::scope::Scope;

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
