use crate::lexer::Lexer;
use std::error::Error;

#[derive(Debug)]
pub struct CrushError {
    pub message: String,
}

pub type CrushResult<T> = Result<T, CrushError>;

pub fn parse_error(message: &str, _lexer: &Lexer) -> CrushError {
    return CrushError {
        message: String::from(message),
    };
}

pub fn argument_error(message: &str) -> CrushError {
    return CrushError {
        message: String::from(message),
    };
}

pub fn error(message: &str) -> CrushError {
    return CrushError {
        message: String::from(message),
    };
}

pub fn to_job_error<T, E: Error>(result: Result<T, E>) -> Result<T, CrushError> {
    match result {
        Ok(v) => Ok(v),
        Err(e) => Err(error(e.description())),
    }
}

pub fn demand<T>(result: Option<T>, desc: &str) -> Result<T, CrushError> {
    match result {
        Some(v) => Ok(v),
        None => Err(error(format!("Missing value for {}", desc).as_str())),
    }
}

pub fn mandate<T>(result: Option<T>, msg: &str) -> Result<T, CrushError> {
    match result {
        Some(v) => Ok(v),
        None => Err(error(msg)),
    }
}
