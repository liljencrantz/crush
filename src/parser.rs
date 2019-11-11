use crate::errors::{parse_error, argument_error, JobResult};
use crate::job::Job;
use crate::lexer::{Lexer, TokenType};
use crate::data::{CellDefinition, ArgumentDefinition, ListDefinition};
use crate::data::CallDefinition;
use regex::Regex;
use std::error::Error;
use crate::glob::Glob;
use crate::closure::Closure;

pub fn parse(lexer: &mut Lexer) -> JobResult<Vec<Job>> {
    let mut jobs: Vec<Job> = Vec::new();
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
            TokenType::EOF | TokenType::ModeEnd => {
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

fn parse_internal(lexer: &mut Lexer) -> JobResult<Job> {
    let mut commands: Vec<CallDefinition> = Vec::new();
    parse_job(lexer, &mut commands)?;
    return Ok(Job::new(commands));
}

fn parse_job(lexer: &mut Lexer, commands: &mut Vec<CallDefinition>) -> JobResult<()> {
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
    for c in s[1..s.len() - 1].chars() {
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

pub fn parse_name(s: &str) -> Option<Vec<Box<str>>> {
    let res = s.split('.').collect::<Vec<&str>>();
    for i in res.iter() {
        if i.is_empty() {
            return None;
        }
    }
    Some(res.iter().map(|e| e.to_string().into_boxed_str()).collect())
}

fn parse_name_from_lexer(lexer: &mut Lexer) -> JobResult<Vec<Box<str>>> {
    let res = match parse_name(&lexer.peek().1[1..]) {
        None => Err(parse_error("Illegal varaible name", lexer)),
        Some(v) => Ok(v),
    };
    lexer.pop();
    res
}

fn parse_command_from_lexer(lexer: &mut Lexer) -> JobResult<Vec<Box<str>>> {
    let res = match parse_name(&lexer.peek().1) {
        None => Err(parse_error("Illegal command name", lexer)),
        Some(v) => Ok(v),
    };
    lexer.pop();
    res
}

fn parse_mode(lexer: &mut Lexer) -> JobResult<Vec<CellDefinition>> {
    let mut cells: Vec<CellDefinition> = Vec::new();
    loop {
        let tt = lexer.peek().0;
        match tt {
            TokenType::ModeEnd => break,
            _ => cells.push(parse_unnamed_argument(lexer)?),
        }
    }
    lexer.pop();
    Ok(cells)
}

fn parse_unnamed_argument(lexer: &mut Lexer) -> JobResult<CellDefinition> {
    let mut cell = parse_unnamed_argument_without_subscript(lexer)?;
    loop {
        if lexer.peek().0 != TokenType::SubscriptStart {
            break;
        }
        lexer.pop();
        let idx = parse_unnamed_argument(lexer)?;
        if lexer.peek().0 != TokenType::SubscriptEnd {
            return Err(parse_error("Expected '['", lexer));
        }
        lexer.pop();
        cell = CellDefinition::Subscript(Box::from(cell), Box::from(idx));
    }
    Ok(cell)
}

fn parse_unnamed_argument_without_subscript(lexer: &mut Lexer) -> JobResult<CellDefinition> {
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
        TokenType::ModeStart => {
            let sigil = lexer.pop().1;
            match sigil {
                "{" => {
                    let dep = parse_internal(lexer)?;
                    lexer.pop();
                    let res = Ok(CellDefinition::JobDefintion(dep));
                    return res;
                }

                "materialized{" => {
                    let dep = parse_internal(lexer)?;
                    lexer.pop();
                    let res = Ok(CellDefinition::MaterializedJobDefintion(dep));
                    return res;
                }

                "`{" => {
                    let dep = parse(lexer)?;
                    lexer.pop();
                    let res = Ok(CellDefinition::ClosureDefinition(Closure::new(dep)));
                    return res;
                }

                "*{" => {
                    match lexer.peek().0 {
                        TokenType::Glob => {
                            let result = Ok(CellDefinition::Glob(Glob::new(lexer.pop().1)));
                            if lexer.peek().0 != TokenType::ModeEnd {
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

                "duration{" => Ok(CellDefinition::Duration(parse_mode(lexer)?)),

                "time{" => Ok(CellDefinition::Time(parse_mode(lexer)?)),

                "list{" => Ok(CellDefinition::List(ListDefinition::new(parse_mode(lexer)?))),

                other => {
                    return Err(parse_error(format!("Cannot handle mode with sigil {}}}", other).as_str(), lexer));
                }
            }
        }

        TokenType::Field => Ok(CellDefinition::Field(parse_name_from_lexer(lexer)?)),
        TokenType::Variable => Ok(CellDefinition::Variable(parse_name_from_lexer(lexer)?)),
        TokenType::Regex => {
            let f = lexer.pop().1;
            let s = &f[2..f.len() - 1];
            match Regex::new(s) {
                Ok(r) => Ok(CellDefinition::regex(s, r)),
                Err(e) => Err(argument_error(e.description())),
            }
        }
        TokenType::QuotedString => Ok(CellDefinition::text(unescape(lexer.pop().1).as_str())),

        TokenType::SubscriptStart => {
            lexer.pop();
            let mut cells: Vec<CellDefinition> = Vec::new();
            loop {
                let tt = lexer.peek().0;
                match tt {
                    TokenType::SubscriptEnd => break,
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

fn parse_argument(lexer: &mut Lexer) -> JobResult<ArgumentDefinition> {
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

fn parse_arguments(lexer: &mut Lexer, arguments: &mut Vec<ArgumentDefinition>) -> JobResult<()> {
    loop {
        match lexer.peek().0 {
            TokenType::Error => {
                return Err(parse_error("Bad token", lexer));
            }
            TokenType::Separator | TokenType::EOF | TokenType::Pipe | TokenType::ModeEnd => {
                return Ok(());
            }
            _ => arguments.push(parse_argument(lexer)?),
        }
    }
}

fn parse_command(lexer: &mut Lexer, commands: &mut Vec<CallDefinition>) -> JobResult<()> {
    match lexer.peek().0 {
        TokenType::String => {
            let name = parse_command_from_lexer(lexer)?;
            let mut arguments: Vec<ArgumentDefinition> = Vec::new();
            parse_arguments(lexer, &mut arguments)?;
            commands.push(CallDefinition::new(name, arguments));
            return Ok(());
        }
        _ => {
            return Err(parse_error("Expected command name", lexer));
        }
    }
}
