use crate::errors::{JobError, parse_error, argument_error};
use crate::job::Job;
use crate::lexer::{Lexer, TokenType};
use crate::state::State;
use crate::cell::{Argument, Cell};
use crate::commands::Call;
use regex::Regex;
use std::error::Error;

pub fn parse(lexer: &mut Lexer, state: &State) -> Result<Vec<Job>, JobError> {
    let mut jobs: Vec<Job> = Vec::new();
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

fn parse_internal(lexer: &mut Lexer, state: &State) -> Result<Job, JobError> {
    let mut commands: Vec<Call> = Vec::new();
    let mut dependencies: Vec<Job> = Vec::new();
    parse_job(lexer, state, &mut commands, &mut dependencies)?;
    return Ok(Job::new(commands, dependencies));
}

fn parse_job(lexer: &mut Lexer, state: &State, commands: &mut Vec<Call>, dependencies: &mut Vec<Job>) -> Result<(), JobError> {
    parse_command(lexer, commands, dependencies, state)?;
    while lexer.peek().0 == TokenType::Pipe {
        lexer.pop();
        parse_command(lexer, commands, dependencies, state)?;
    }
    return Ok(());
}

fn parse_unnamed_argument(lexer: &mut Lexer, dependencies: &mut Vec<Job>, state: &State) -> Result<Cell, JobError> {
    let token_type = lexer.peek().0;
    match token_type {
        TokenType::String => {
            return Ok(Cell::Text(String::from(lexer.pop().1)));
        }
        TokenType::Glob => {
            return Ok(Cell::Glob(String::from(lexer.pop().1)));
        }
        TokenType::Integer => {
            return match String::from(lexer.pop().1).parse::<i128>() {
                Ok(ival) => Ok(Cell::Integer(ival)),
                Err(_) => Err(parse_error("Invalid number", lexer)),
            };
        }
        TokenType::Equal | TokenType::NotEqual | TokenType::GreaterThan
        | TokenType::GreaterThanOrEqual | TokenType::LessThan | TokenType::LessThanOrEqual
        | TokenType::Match | TokenType::NotMatch => {
            return Ok(Cell::Op(String::from(lexer.pop().1)));
        }
        TokenType::BlockStart => {
            let sigil_type = lexer.pop().1.chars().next().unwrap();
            match sigil_type {
                '{' => {
                    let mut dep = parse_internal(lexer, state)?;
                    lexer.pop();
                    let res = Ok(Cell::Output(dep.take_output().unwrap()));
                    dependencies.push(dep);
                    return res;
                }
                '*' => {
                    match lexer.peek().0 {
                        TokenType::Glob => {
                            let result = Ok(Cell::Glob(String::from(lexer.pop().1)));
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
        },

        TokenType::Field => Ok(Cell::Field(String::from(&lexer.pop().1[1..]))),
        TokenType::Variable => match state.namespace.get(&lexer.pop().1[1..]) {
            Some(cell) => Ok(cell.partial_clone().unwrap()),
            None => Err(parse_error("Unknown variable", lexer)),
        }
        TokenType::Regex => {
            let f = lexer.pop().1;
            let s = &f[2..f.len() - 1];
            match Regex::new(s) {
               Ok(r) => Ok(Cell::Regex(String::from(s), r)),
                Err(e) => Err(argument_error(e.description())),
            }
        },

        _ => {
            lexer.pop();
            return Err(parse_error(format!("Unknown token {:?}", token_type).as_str(), lexer));
        }
    }
}

fn parse_arguments(lexer: &mut Lexer, arguments: &mut Vec<Argument>, dependencies: &mut Vec<Job>, state: &State) -> Result<(), JobError> {
    loop {
        match lexer.peek().0 {
            TokenType::Error => {
                return Err(parse_error("Bad token", lexer));
            }
            TokenType::Separator | TokenType::EOF | TokenType::Pipe | TokenType::BlockEnd => {
                return Ok(());
            }
            TokenType::String => {
                let ss = String::from(lexer.pop().1);
                if lexer.peek().0 == TokenType::Assign {
                    lexer.pop();
                    arguments.push(Argument::named(&ss, parse_unnamed_argument(lexer, dependencies, state)?));
                } else {
                    arguments.push(Argument::unnamed(Cell::Text(ss)));
                }
            }
            _ => {
                arguments.push(Argument::unnamed(parse_unnamed_argument(lexer, dependencies, state)?));
            }
        }
    }
}

fn parse_command(lexer: &mut Lexer, commands: &mut Vec<Call>, dependencies: &mut Vec<Job>, state: &State) -> Result<(), JobError> {
    let empty_vec = Vec::new();
    let input = match commands.last() {
        Some(cmd) => { cmd.get_output_type() }
        None => { &empty_vec }
    };

    match lexer.peek().0 {
        TokenType::String => {
            let name = String::from(lexer.pop().1);
            let mut arguments: Vec<Argument> = Vec::new();
            parse_arguments(lexer, &mut arguments, dependencies, state)?;
            let call = state.namespace.call(&name, input.clone(), arguments)?;
            commands.push(call);
            return Ok(());
        }
        _ => {
            return Err(parse_error("Expected command name", lexer));
        }
    }
}
