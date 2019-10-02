use crate::errors::JobError;
use crate::job::Job;
use crate::lexer::{Lexer, TokenType};
use crate::state::State;
use crate::result::Argument;

pub fn parse_job(lexer: &mut Lexer, job: &mut Job, state: &State) -> Result<(), JobError> {
    parse_command(lexer, job)?;
    while(lexer.peek_type() == TokenType::Pipe) {
        lexer.pop();
        parse_command(lexer, job, state)?;
    }
    return Ok(());
}

pub fn parse_command(lexer: &mut Lexer, job: &mut Job, state: &State) -> Result<(), JobError> {
    match lexer.pop() {
        Some((TokenType::String, name)) => {
            let arguments: Vec<Argument> = Vec::new();
            state.commands.call(&String::from(name), input, &arguments);
        }
    }
    return Ok(());
}
