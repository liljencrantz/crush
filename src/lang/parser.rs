use crate::lang::errors::{parse_error, argument_error, CrushResult, to_crush_error};
use crate::lang::job::Job;
use crate::lang::lexer::{Lexer, TokenType};
use crate::lang::{value::ValueDefinition, argument::ArgumentDefinition};
use crate::lang::call_definition::CallDefinition;
use std::error::Error;

/*
job_list := | non_empty_job_list

non_empty_job_list := non_empty_job_list Separator job | job

job := command | job Pipe command

command := expression | command expression

expression := assignment_expression | item assignment_list | '[' job_list ']' | '(' job ')'

assignment_list := | non_empty_assigment_list

non_empty_assigment_list := assignment | non_empty_assignment_list assignment

assignment_expression := label assignment_op assignment_expression | expression1 '[' job ']' '=' job | logical_expression;

logical_expression := logical_expression logical_op comparison_expression | comparsion_expression

comparison_expression := comparison_expression comparison_op term | term

term := term add_op factor | factor

factor := factor mul_op unary_expression

unary_expression := unary_op item

item := label | item [ job ] | item '/' label

*/

lalrpop_mod!(pub parser2, "/lang/lalrparser.rs");

pub fn parse_name(s: &str) -> Option<Vec<Box<str>>> {
    let res = s.split('.').collect::<Vec<&str>>();
    for i in res.iter() {
        if i.is_empty() {
            return None;
        }
    }
    Some(res.iter().map(|e| e.to_string().into_boxed_str()).collect())
}

pub fn parse(s: &str) -> CrushResult<Vec<Job>> {
    to_crush_error(parser2::JobListParser::new().parse(s))?.generate()
}
