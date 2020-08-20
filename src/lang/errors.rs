use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Eq)]
pub enum CrushError {
    InvalidArgument(String),
    InvalidData(String),
    GenericError(String),
    BlockError,
    SendError,
    EOFError,
}

impl CrushError {
    pub fn message(&self) -> String {
        match self {
            CrushError::InvalidArgument(s)
            | CrushError::InvalidData(s)
            | CrushError::GenericError(s) => s.clone(),
            CrushError::BlockError => "Block error".to_string(),
            CrushError::SendError => "Send error".to_string(),
            CrushError::EOFError => "EOF error".to_string(),
        }
    }
}

impl<T: Display> From<T> for CrushError {
    fn from(message: T) -> Self {
        CrushError::GenericError(message.to_string())
    }
}

pub type CrushResult<T> = Result<T, CrushError>;

pub fn block_error<T>() -> Result<T, CrushError> {
    Err(CrushError::BlockError)
}

pub fn eof_error<T>() -> Result<T, CrushError> {
    Err(CrushError::EOFError)
}

pub fn send_error<T>() -> Result<T, CrushError> {
    Err(CrushError::SendError)
}

pub fn argument_error<T>(message: impl Into<String>) -> Result<T, CrushError> {
    Err(CrushError::InvalidArgument(message.into()))
}

pub fn data_error<T>(message: impl Into<String>) -> Result<T, CrushError> {
    Err(CrushError::InvalidData(message.into()))
}

pub fn error<T>(message: impl Into<String>) -> Result<T, CrushError> {
    Err(CrushError::GenericError(message.into()))
}

pub fn to_crush_error<T, E: Error>(result: Result<T, E>) -> Result<T, CrushError> {
    match result {
        Ok(v) => Ok(v),
        Err(e) => error(e.to_string()),
    }
}

pub fn mandate<T>(result: Option<T>, msg: impl Into<String>) -> Result<T, CrushError> {
    match result {
        Some(v) => Ok(v),
        None => error(msg),
    }
}
