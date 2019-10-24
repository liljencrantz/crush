use crate::errors::{JobError, parse_error, argument_error};
use crate::job::JobDefinition;
use crate::lexer::{Lexer, TokenType};
use crate::env::Env;
use crate::data::{CellDefinition, ArgumentDefinition, Cell, ListDefinition};
use crate::data::CallDefinition;
use regex::Regex;
use std::error::Error;
use crate::glob::Glob;
use crate::closure::{Closure, ClosureDefinition};

pub fn parse(lexer: &mut Lexer) -> Result<Vec<JobDefinition>, JobError> {
    let mut jobs: Vec<JobDefinition> = Vec::new();
    loop {
        match lexer.peek() {
            (TokenType::String, _) => {
                jobs.push(parse_internal(lexer)?);
            }
            _ => {
                return Err(parse_error("Wrong token type, expected command name", lexer));
            }
        }

        match lexer.peek().0 {
            TokenType::EOF | TokenType::BlockEnd => {
                return Ok(jobs);
            }
            TokenType::Error => {
                return Err(parse_error("Bad token", lexer));
            }
            TokenType::Separator => {
                lexer.pop();
            }
            _ => {
                return Err(parse_error("Wrong token type", lexer));
            }
        }
    }
}

fn parse_internal(lexer: &mut Lexer) -> Result<JobDefinition, JobError> {
    let mut commands: Vec<CallDefinition> = Vec::new();
    parse_job(lexer, &mut commands)?;
    return Ok(JobDefinition::new(commands));
}

fn parse_job(lexer: &mut Lexer, commands: &mut Vec<CallDefinition>) -> Result<(), JobError> {
    parse_command(lexer, commands)?;
    while lexer.peek().0 == TokenType::Pipe {
        lexer.pop();
        parse_command(lexer, commands)?;
    }
    return Ok(());
}

fn unescape(s: &str) -> String {
    let mut res = "".to_string();
    let mut was_backslash = false;
    for c in s[1..s.len()-1].chars() {
        if was_backslash {
            match c {
                'n' => res += "\n",
                'r' => res += "\r",
                't' => res += "\t",
                _ => res += &c.to_string(),
            }
        } else {
            if c == '\\' {
                was_backslash = true;
            } else {
                res += &c.to_string();
            }
        }
    }
    res
}

fn parse_unnamed_argument(lexer: &mut Lexer) -> Result<CellDefinition, JobError> {
    let token_type = lexer.peek().0;
    match token_type {
        TokenType::String => {
            return Ok(CellDefinition::text(lexer.pop().1));
        }
        TokenType::Glob => {
            return Ok(CellDefinition::Glob(Glob::new(lexer.pop().1)));
        }
        TokenType::Integer => {
            return match String::from(lexer.pop().1).parse::<i128>() {
                Ok(ival) => Ok(CellDefinition::Integer(ival)),
                Err(_) => Err(parse_error("Invalid number", lexer)),
            };
        }
        TokenType::Equal | TokenType::NotEqual | TokenType::GreaterThan
        | TokenType::GreaterThanOrEqual | TokenType::LessThan | TokenType::LessThanOrEqual
        | TokenType::Match | TokenType::NotMatch => {
            return Ok(CellDefinition::op(lexer.pop().1));
        }
        TokenType::BlockStart => {
            let sigil_type = lexer.pop().1.chars().next().unwrap();
            match sigil_type {
                '{' => {
                    let mut dep = parse_internal(lexer)?;
                    lexer.pop();
                    let res = Ok(CellDefinition::JobDefintion(dep));
                    return res;
                }
                '`' => {
                    let mut dep = parse(lexer)?;
                    lexer.pop();
                    let res = Ok(CellDefinition::ClosureDefinition(ClosureDefinition::new(dep)));
                    return res;
                }
                '*' => {
                    match lexer.peek().0 {
                        TokenType::Glob => {
                            let result = Ok(CellDefinition::Glob(Glob::new(lexer.pop().1)));
                            if lexer.peek().0 != TokenType::BlockEnd {
                                return Err(parse_error("Expected '}'", lexer));
                            }
                            lexer.pop();
                            return result;
                        }
                        _ => {
                            return Err(parse_error("Expected string token", lexer));
                        }
                    }
                }
                _ => {
                    return Err(parse_error("Cannot handle sigil type", lexer));
                }
            }
        }

        TokenType::Field => Ok(CellDefinition::field(&lexer.pop().1[1..])),
        TokenType::Variable => Ok(CellDefinition::Variable(lexer.pop().1[1..].to_string().into_boxed_str())),
        TokenType::Regex => {
            let f = lexer.pop().1;
            let s = &f[2..f.len() - 1];
            match Regex::new(s) {
                Ok(r) => Ok(CellDefinition::regex(s, r)),
                Err(e) => Err(argument_error(e.description())),
            }
        }
        TokenType::QuotedString => Ok(CellDefinition::text(unescape(lexer.pop().1).as_str())),

        TokenType::ListStart => {
            lexer.pop();
            let mut cells: Vec<CellDefinition> = Vec::new();
            loop {
                let tt = lexer.peek().0;
                match tt {
                TokenType::ListEnd => break,
                _ => cells.push(parse_unnamed_argument(lexer)?),
                }
            }
            lexer.pop();
            Ok(CellDefinition::List(ListDefinition::new(cells)))
        }

        _ => {
            lexer.pop();
            return Err(parse_error(format!("Unknown token {:?}", token_type).as_str(), lexer));
        }
    }
}

fn parse_argument(lexer: &mut Lexer) -> Result<ArgumentDefinition, JobError> {
    match lexer.peek().0 {
        TokenType::String => {
            let ss = lexer.pop().1.to_string();
            if lexer.peek().0 == TokenType::Assign {
                lexer.pop();
                return Ok(ArgumentDefinition::named(&ss, parse_unnamed_argument(lexer)?));
            } else {
                return Ok(ArgumentDefinition::unnamed(CellDefinition::text(ss.as_str())));
            }
        }
        _ => {
            return Ok(ArgumentDefinition::unnamed(parse_unnamed_argument(lexer)?));
        }
    }
}

fn parse_arguments(lexer: &mut Lexer, arguments: &mut Vec<ArgumentDefinition>) -> Result<(), JobError> {
    loop {
        match lexer.peek().0 {
            TokenType::Error => {
                return Err(parse_error("Bad token", lexer));
            }
            TokenType::Separator | TokenType::EOF | TokenType::Pipe | TokenType::BlockEnd => {
                return Ok(());
            }
            _ => arguments.push(parse_argument(lexer)?),
        }
    }
}

fn parse_command(lexer: &mut Lexer, commands: &mut Vec<CallDefinition>) -> Result<(), JobError> {

    match lexer.peek().0 {
        TokenType::String => {
            let name = String::from(lexer.pop().1);
            let mut arguments: Vec<ArgumentDefinition> = Vec::new();
            parse_arguments(lexer, &mut arguments)?;
            commands.push(CallDefinition::new(name.as_str(), arguments));
            return Ok(());
        }
        _ => {
            return Err(parse_error("Expected command name", lexer));
        }
    }
}
