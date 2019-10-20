use crate::errors::{JobError, parse_error, argument_error};
use crate::job::JobDefinition;
use crate::lexer::{Lexer, TokenType};
use crate::state::State;
use crate::data::{CellDefinition, ArgumentDefinition, ConcreteCell};
use crate::commands::CallDefinition;
use regex::Regex;
use std::error::Error;
use crate::data::Cell;
use crate::glob::Glob;

pub fn parse(lexer: &mut Lexer, state: &State) -> Result<Vec<JobDefinition>, JobError> {
    let mut jobs: Vec<JobDefinition> = Vec::new();
    loop {
        match lexer.peek() {
            (TokenType::String, _) => {
                jobs.push(parse_internal(lexer, state)?);
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

fn parse_internal(lexer: &mut Lexer, state: &State) -> Result<JobDefinition, JobError> {
    let mut commands: Vec<CallDefinition> = Vec::new();
    parse_job(lexer, state, &mut commands)?;
    return Ok(JobDefinition::new(commands));
}

fn parse_job(lexer: &mut Lexer, state: &State, commands: &mut Vec<CallDefinition>) -> Result<(), JobError> {
    parse_command(lexer, commands, state)?;
    while lexer.peek().0 == TokenType::Pipe {
        lexer.pop();
        parse_command(lexer, commands, state)?;
    }
    return Ok(());
}

fn unescape(s: &str) -> String {
    s[1..s.len() - 1].to_string()
}

fn parse_unnamed_argument(lexer: &mut Lexer, state: &State) -> Result<CellDefinition, JobError> {
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
                    let mut dep = parse_internal(lexer, state)?;
                    lexer.pop();
                    let res = Ok(CellDefinition::JobDefintion(dep));
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
        TokenType::Variable => match state.namespace.get(&lexer.pop().1[1..]) {
            Some(cell) => Ok(cell.clone().to_cell_definition()),
            None => Err(parse_error("Unknown variable", lexer)),
        }
        TokenType::Regex => {
            let f = lexer.pop().1;
            let s = &f[2..f.len() - 1];
            match Regex::new(s) {
                Ok(r) => Ok(CellDefinition::regex(s, r)),
                Err(e) => Err(argument_error(e.description())),
            }
        }
        TokenType::QuotedString => Ok(CellDefinition::text(unescape(lexer.pop().1).as_str())),

        _ => {
            lexer.pop();
            return Err(parse_error(format!("Unknown token {:?}", token_type).as_str(), lexer));
        }
    }
}

fn parse_argument(lexer: &mut Lexer, state: &State) -> Result<ArgumentDefinition, JobError> {
    match lexer.peek().0 {
        TokenType::String => {
            let ss = lexer.pop().1.to_string();
            if lexer.peek().0 == TokenType::Assign {
                lexer.pop();
                return Ok(ArgumentDefinition::named(&ss, parse_unnamed_argument(lexer, state)?));
            } else {
                return Ok(ArgumentDefinition::unnamed(CellDefinition::text(ss.as_str())));
            }
        }
        _ => {
            return Ok(ArgumentDefinition::unnamed(parse_unnamed_argument(lexer, state)?));
        }
    }
}

fn parse_arguments(lexer: &mut Lexer, arguments: &mut Vec<ArgumentDefinition>, state: &State) -> Result<(), JobError> {
    loop {
        match lexer.peek().0 {
            TokenType::Error => {
                return Err(parse_error("Bad token", lexer));
            }
            TokenType::Separator | TokenType::EOF | TokenType::Pipe | TokenType::BlockEnd => {
                return Ok(());
            }
            _ => arguments.push(parse_argument(lexer, state)?),
        }
    }
}

fn parse_command(lexer: &mut Lexer, commands: &mut Vec<CallDefinition>, state: &State) -> Result<(), JobError> {

    match lexer.peek().0 {
        TokenType::String => {
            let name = String::from(lexer.pop().1);
            let mut arguments: Vec<ArgumentDefinition> = Vec::new();
            parse_arguments(lexer, &mut arguments, state)?;
            match &state.namespace.get(&name) {
                Some(ConcreteCell::Command(command)) => {
                    commands.push(CallDefinition { arguments, command: command.clone() });
                }
                _ => {
                    return Err(parse_error("Expected command name", lexer));
                }
            }
            return Ok(());
        }
        _ => {
            return Err(parse_error("Expected command name", lexer));
        }
    }
}
