use crate::errors::{JobError, parse_error};
use crate::job::{Job, JobState};
use crate::lexer::{Lexer, TokenType};
use crate::state::State;
use crate::cell::{Argument, Cell};

pub fn parse(lexer: &mut Lexer, state: &State) -> Result<Vec<Job>, JobError> {
    let mut jobs: Vec<Job> = Vec::new();
    parse_internal(lexer, state, &mut jobs)?;
    return Ok(jobs);
}

fn parse_internal(lexer: &mut Lexer, state: &State, jobs: &mut Vec<Job>) -> Result<(), JobError> {
    loop {
        match lexer.peek() {
            (TokenType::String, _) => {
                jobs.push(Job::new());
                let idx = jobs.len() - 1;
                parse_job(lexer, state, &mut jobs[idx])?;
                jobs[idx].state = JobState::Parsed;
            }
            _ => {
                return Err(parse_error("Wrong token type, expected command name", lexer));
            }
        }

        match lexer.peek().0 {
            TokenType::EOF => {
                return Ok(());
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

fn parse_job(lexer: &mut Lexer, state: &State, job: &mut Job) -> Result<(), JobError> {
    parse_command(lexer, job, state)?;
    while lexer.peek().0 == TokenType::Pipe {
        lexer.pop();
        parse_command(lexer, job, state)?;
    }
    return Ok(());
}

fn parse_unnamed_argument(lexer: &mut Lexer, state: &State) -> Result<Cell, JobError> {
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
        | TokenType::GreaterThanOrEqual | TokenType::LessThan | TokenType::LessThanOrEqual => {
            return Ok(Cell::Op(String::from(lexer.pop().1)));
        }
        TokenType::BlockStart => {
            let sigil_type = lexer.pop().1.chars().next().unwrap();
            match sigil_type {
                '%' => {
                    match lexer.peek().0 {
                        TokenType::String => {
                            let result = Ok(Cell::Field(String::from(lexer.pop().1)));
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
                'r' => {
                    match lexer.peek().0 {
                        TokenType::String => {
                            let result = Ok(Cell::Regex(String::from(lexer.pop().1)));
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
        _ => {
            lexer.pop();
            return Err(parse_error(format!("Unknown token {:?}", token_type).as_str(), lexer));
        }
    }
}

fn parse_arguments(lexer: &mut Lexer, arguments: &mut Vec<Argument>, state: &State) -> Result<(), JobError> {
    loop {
        match lexer.peek().0 {
            TokenType::Error => {
                return Err(parse_error("Bad token", lexer));
            }
            TokenType::Separator | TokenType::EOF | TokenType::Pipe => {
                return Ok(());
            }
            TokenType::String => {
                let ss = String::from(lexer.pop().1);
                if lexer.peek().0 == TokenType::Assign {
                    lexer.pop();
                    arguments.push(Argument::named(&ss, &parse_unnamed_argument(lexer, state)?));
                } else {
                    arguments.push(Argument::unnamed(&Cell::Text(ss)));
                }
            }
            _ => {
                arguments.push(Argument::unnamed(&parse_unnamed_argument(lexer, state)?));
            }
        }
    }
}

fn parse_command(lexer: &mut Lexer, job: &mut Job, state: &State) -> Result<(), JobError> {
    let empty_vec = Vec::new();
    let input = match job.commands.last() {
        Some(cmd) => { cmd.get_output_type() }
        None => { &empty_vec }
    };

    match lexer.peek().0 {
        TokenType::String => {
            let name = String::from(lexer.pop().1);
            let mut arguments: Vec<Argument> = Vec::new();
            parse_arguments(lexer, &mut arguments, state)?;
            let call = state.namespace.call(&name, input, &arguments)?;
            job.commands.push(call);
            return Ok(());
        }
        _ => {
            return Err(parse_error("Expected command name", lexer));
        }
    }
}
