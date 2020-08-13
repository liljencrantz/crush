use crate::lang::errors::Kind::*;
use std::error::Error;

#[derive(Debug, PartialEq)]
pub enum Kind {
    //    ParseError,
    InvalidArgument,
    InvalidData,
    GenericError,
    BlockError,
    SendError,
}

#[derive(Debug)]
pub struct CrushError {
    pub kind: Kind,
    pub message: String,
}

pub type CrushResult<T> = Result<T, CrushError>;

pub fn block_error<T>() -> Result<T, CrushError> {
    Err(CrushError {
        message: String::from(
            "Internal error: Tried to call blocking code in a thread that may not block",
        ),
        kind: BlockError,
    })
}

pub fn send_error<T>() -> Result<T, CrushError> {
    Err(CrushError {
        message: String::from("Tried to send data to a command that is no longer listening. This is almost normal behaviour and can be safely ignored."),
        kind: SendError,
    })
}

pub fn argument_error<T>(message: &str) -> Result<T, CrushError> {
    Err(CrushError {
        message: String::from(message),
        kind: InvalidArgument,
    })
}

pub fn data_error<T>(message: &str) -> Result<T, CrushError> {
    Err(CrushError {
        message: String::from(message),
        kind: InvalidData,
    })
}

pub fn error<T>(message: impl Into<String>) -> Result<T, CrushError> {
    Err(CrushError {
        message: message.into(),
        kind: GenericError,
    })
}

pub fn to_crush_error<T, E: Error>(result: Result<T, E>) -> Result<T, CrushError> {
    match result {
        Ok(v) => Ok(v),
        Err(e) => error(e.to_string().as_str()),
    }
}

pub fn mandate<T>(result: Option<T>, msg: &str) -> Result<T, CrushError> {
    match result {
        Some(v) => Ok(v),
        None => error(msg),
    }
}
