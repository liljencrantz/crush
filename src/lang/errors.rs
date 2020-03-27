use std::error::Error;
use crate::lang::errors::Kind::*;

#[derive(Debug, PartialEq)]
pub enum Kind {
//    ParseError,
    InvalidArgument,
    InvalidData,
    GenericError,
    BlockError,
}

#[derive(Debug)]
pub struct CrushError {
    pub kind: Kind,
    pub message: String,
}

pub type CrushResult<T> = Result<T, CrushError>;

pub fn block_error<T>() -> Result<T, CrushError> {
    return Err(CrushError {
        message: String::from("Internal error: Tried to call blocking code in a thread that may not block"),
        kind: BlockError,
    });
}

pub fn argument_error<T>(message: &str) -> Result<T, CrushError> {
    return Err(CrushError {
        message: String::from(message),
        kind: InvalidArgument,
    });
}

pub fn data_error<T>(message: &str) -> Result<T, CrushError> {
    return Err(CrushError {
        message: String::from(message),
        kind: InvalidData,
    });
}

pub fn error<T>(message: &str) -> Result<T, CrushError> {
    return Err(CrushError {
        message: String::from(message),
        kind: GenericError,
    });
}

pub fn to_crush_error<T, E: Error>(result: Result<T, E>) -> Result<T, CrushError> {
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
