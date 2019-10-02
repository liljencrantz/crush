use crate::errors::JobError;
use crate::job::Job;
use crate::lexer::{Lexer, TokenType};
use crate::state::State;
use crate::result::{Argument, Cell};

pub fn parse(lexer: &mut Lexer, state: &State) -> Result<Vec<Job>, JobError> {
    let mut jobs: Vec<Job> = Vec::new();
    parse_internal(lexer, state, &mut jobs)?;
    return Ok(jobs);
}

fn parse_internal(lexer: &mut Lexer, state: &State, jobs: &mut Vec<Job>) -> Result<(), JobError> {
    loop {
        match lexer.peek() {
            (TokenType::String, s) => {
                jobs.push(Job::new());
                let idx = jobs.len() - 1;
                return parse_job(lexer, state, &mut jobs[idx]);
            }
            _ => {
                return Err(JobError::parse_error(&String::from("Wrong token type, expected command name"), lexer));
            }
        }

        match lexer.peek().0 {
            TokenType::EOF => {
                return Ok(());
            }
            TokenType::Error => {
                return Err(JobError::parse_error(&String::from("Wrong token type"), lexer));
            }
            _ => {}
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

fn parse_arguments(lexer: &mut Lexer, arguments: &mut Vec<Argument>, state: &State) -> Result<(), JobError> {
    loop {
        match lexer.peek().0 {
            TokenType::Error => {
                return Err(JobError::parse_error(&String::from("Unknown token"), lexer));
            }
            TokenType::Separator | TokenType::EOF | TokenType::Pipe => {
                return Ok(());
            }
            TokenType::String => {
                let ss = String::from(lexer.pop().1);
                if lexer.peek().0 == TokenType::Assign {
                    lexer.pop();
                    match lexer.peek().0 {
                        TokenType::String => {
                            arguments.push(Argument::named(&ss, &Cell::Text(String::from(lexer.pop().1))));
                        }
                        _ => {
                            return Err(JobError::parse_error(&String::from("Unknown token"), lexer));
                        }
                    }
                } else {
                    arguments.push(Argument::unnamed(&Cell::Text(ss)));
                }
            }
            _ => {
                lexer.pop();
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
            let call = state.commands.call(&name, input, &arguments)?;
            job.commands.push(call);
            return Ok(());
        }
        _ => {
            return Err(JobError { message: String::from("Expected command name") });
        }
    }
}
