use crate::lexer::Lexer;
use std::error::Error;
use std::io;

#[derive(Debug)]
pub struct JobError {
    pub message: String,
}

pub fn parse_error(message: &str, _lexer: &Lexer) -> JobError {
    return JobError {
        message: String::from(message),
    };
}

pub fn argument_error(message: &str) -> JobError {
    return JobError {
        message: String::from(message),
    };
}

pub fn error(message: &str) -> JobError {
    return JobError {
        message: String::from(message),
    };
}

pub fn to_runtime_error<T>(io_result: io::Result<T>) -> Result<T, JobError> {
    match io_result {
        Ok(v) => Ok(v),
        Err(e) => Err(error(e.description())),
    }
}
