use crate::lexer::Lexer;
use std::error::Error;
use crate::errors::Kind::{PARSE_ERROR, INVALID_ARGUMENT, GENERIC_ERROR, INVALID_DATA};

#[derive(Debug)]
pub enum Kind {
    PARSE_ERROR,
    INVALID_ARGUMENT,
    INVALID_DATA,
    GENERIC_ERROR,
}

#[derive(Debug)]
pub struct CrushError {
    pub kind: Kind,
    pub message: String,
}

pub type CrushResult<T> = Result<T, CrushError>;

pub fn parse_error<T>(message: &str, _lexer: &Lexer) -> Result<T, CrushError> {
    return Err(CrushError {
        message: String::from(message),
        kind: PARSE_ERROR,
    });
}

pub fn argument_error<T>(message: &str) -> Result<T, CrushError> {
    return Err(CrushError {
        message: String::from(message),
        kind: INVALID_ARGUMENT,
    });
}

pub fn data_error<T>(message: &str) -> Result<T, CrushError> {
    return Err(CrushError {
        message: String::from(message),
        kind: INVALID_DATA,
    });
}

pub fn error<T>(message: &str) -> Result<T, CrushError> {
    return Err(CrushError {
        message: String::from(message),
        kind: GENERIC_ERROR,
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
