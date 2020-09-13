use std::error::Error;
use std::fmt::{Display};
use crate::lang::ast::Location;
use CrushErrorType::*;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CrushErrorType {
    InvalidArgument(String),
    InvalidData(String),
    GenericError(String),
    BlockError,
    SendError,
    EOFError,
}

#[derive(Debug, Clone)]
pub struct CrushError {
    error_type: CrushErrorType,
    location: Option<Location>,
    definition: Option<String>,
}

impl CrushError {
    pub fn is(&self, t: CrushErrorType) -> bool {
        self.error_type == t
    }

    pub fn is_eof(&self) -> bool {
        self.error_type == CrushErrorType::EOFError
    }

    pub fn message(&self) -> String {
        match &self.error_type {
            InvalidArgument(s)
            | InvalidData(s)
            | GenericError(s) => s.clone(),
            BlockError => "Block error".to_string(),
            SendError => "Send error".to_string(),
            EOFError => "EOF error".to_string(),
        }
    }

    pub fn location(&self) -> Option<Location> {
        self.location
    }

    pub fn with_source(&self, source: &Option<(String, Location)>) -> CrushError {
        match source {
            None => self.clone(),
            Some((def, loc)) =>
                self.with_location(*loc).with_definition(def),
        }
    }

    pub fn with_definition(&self, def: impl Into<String>) -> CrushError {
        CrushError {
            error_type: self.error_type.clone(),
            location: self.location,
            definition: Some(def.into()),
        }
    }

    pub fn with_location(&self, l: Location) -> CrushError {
        let location = if let Some(old) = self.location() {
            if old.len() < l.len() {
                old
            } else {
                l
            }
        } else {
            l
        };
        CrushError {
            error_type: self.error_type.clone(),
            location: Some(location),
            definition: self.definition.clone(),
        }
    }

    pub fn context(&self) -> Option<String> {
        match (&self.definition, self.location) {
            (Some(def), Some(loc)) => {
                Some(format!("{}\n{}{}", def, " ".repeat(loc.start), "^".repeat(loc.len())))
            }
            _ => None
        }
    }
}

impl<T: Display> From<T> for CrushError {
    fn from(message: T) -> Self {
        CrushError {
            error_type: GenericError(message.to_string()),
            location: None,
            definition: None,
        }
    }
}

pub type CrushResult<T> = Result<T, CrushError>;

pub fn block_error<T>() -> Result<T, CrushError> {
    Err(CrushError {
        error_type: BlockError,
        location: None,
        definition: None,
    })
}

pub fn eof_error<T>() -> CrushResult<T> {
    Err(CrushError {
        error_type: EOFError,
        location: None,
        definition: None,
    })
}

pub fn send_error<T>() -> CrushResult<T> {
    Err(CrushError {
        error_type: SendError,
        location: None,
        definition: None,
    })
}

pub fn argument_error_legacy<T>(message: impl Into<String>) -> CrushResult<T> {
    Err(CrushError {
        error_type: InvalidArgument(message.into()),
        location: None,
        definition: None,
    })
}

pub fn argument_error<T>(message: impl Into<String>, location: Location) -> CrushResult<T> {
    Err(CrushError {
        error_type: InvalidArgument(message.into()),
        location: Some(location),
        definition: None,
    })
}

pub fn data_error<T>(message: impl Into<String>) -> CrushResult<T> {
    Err(CrushError {
        error_type: InvalidData(message.into()),
        location: None,
        definition: None,
    })
}

pub fn error<T>(message: impl Into<String>) -> CrushResult<T> {
    Err(CrushError {
        error_type: GenericError(message.into()),
        location: None,
        definition: None,
    })
}

pub fn to_crush_error<T, E: Error>(result: Result<T, E>) -> Result<T, CrushError> {
    match result {
        Ok(v) => Ok(v),
        Err(e) => error(e.to_string()),
    }
}

pub fn mandate<T>(result: Option<T>, msg: impl Into<String>) -> CrushResult<T> {
    match result {
        Some(v) => Ok(v),
        None => data_error(msg),
    }
}
