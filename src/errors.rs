use crate::lexer::Lexer;
use std::error::Error;

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

pub fn to_job_error<T, E: Error>(result: Result<T, E>) -> Result<T, JobError> {
    match result {
        Ok(v) => Ok(v),
        Err(e) => Err(error(e.description())),
    }
}

pub fn mandate<T>(result: Option<T>) -> Result<T, JobError> {
    match result {
        Some(v) => Ok(v),
        None => Err(error("Missing value")),
    }
}
