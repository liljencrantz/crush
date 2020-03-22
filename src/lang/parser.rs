use crate::lang::errors::{parse_error, argument_error, CrushResult, to_crush_error};
use crate::lang::job::Job;
use crate::lang::lexer::{Lexer, TokenType};
use crate::lang::{value::ValueDefinition, argument::ArgumentDefinition};
use crate::lang::command_invocation::CommandInvocation;
use std::error::Error;

lalrpop_mod!(pub lalrparser, "/lang/lalrparser.rs");

pub fn parse_name(s: &str) -> Option<Vec<Box<str>>> {
    let res = s.split('/').collect::<Vec<&str>>();
    for i in res.iter() {
        if i.is_empty() {
            return None;
        }
    }
    Some(res.iter().map(|e| e.to_string().into_boxed_str()).collect())
}

pub fn parse(s: &str) -> CrushResult<Vec<Job>> {
    to_crush_error(lalrparser::JobListParser::new().parse(s))?.generate()
}
