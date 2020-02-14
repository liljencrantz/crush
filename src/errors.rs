use crate::lexer::Lexer;
use std::error::Error;

#[derive(Debug)]
pub struct CrushError {
    pub message: String,
}

pub type CrushResult<T> = Result<T, CrushError>;

pub fn parse_error<T>(message: &str, _lexer: &Lexer) -> Result<T, CrushError> {
    return Err(CrushError {
        message: String::from(message),
    });
}

pub fn argument_error<T>(message: &str) -> Result<T, CrushError> {
    return Err(CrushError {
        message: String::from(message),
    });
}

pub fn error<T>(message: &str) -> Result<T, CrushError> {
    return Err(CrushError {
        message: String::from(message),
    });
}

pub fn to_job_error<T, E: Error>(result: Result<T, E>) -> Result<T, CrushError> {
    match result {
        Ok(v) => Ok(v),
        Err(e) => error(e.description()),
    }
}

pub fn demand<T>(result: Option<T>, desc: &str) -> Result<T, CrushError> {
    match result {
        Some(v) => Ok(v),
        None => error(format!("Missing value for {}", desc).as_str()),
    }
}

pub fn mandate<T>(result: Option<T>, msg: &str) -> Result<T, CrushError> {
    match result {
        Some(v) => Ok(v),
        None => error(msg),
    }
}
