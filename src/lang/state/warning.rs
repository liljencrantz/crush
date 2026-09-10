use crate::lang::ast::source::Source;
use crate::lang::errors::CrushError;
use chrono::{DateTime, Local};

/// A non-fatal problem reported by a command that experienced a partial failure --
/// e.g. one row out of a stream failed to process, but the command as a whole
/// continued and succeeded. Distinct from a CrushError, which always aborts the
/// command that produced it; a Warning is something a command chooses to report
/// and move past.
#[derive(Clone, Debug)]
pub struct Warning {
    message: String,
    command: Option<String>,
    source: Option<Source>,
    timestamp: DateTime<Local>,
}

impl Warning {
    pub fn new(
        message: impl Into<String>,
        command: Option<String>,
        source: Option<Source>,
    ) -> Warning {
        Warning {
            message: message.into(),
            command,
            source,
            timestamp: Local::now(),
        }
    }

    /// Build a Warning from a CrushError that was going to be swallowed anyway --
    /// reuses whatever message/command/source it already carries instead of making
    /// the caller reconstruct them by hand.
    pub fn from_error(err: &CrushError) -> Warning {
        Warning {
            message: err.message(),
            command: err.command().clone(),
            source: err.source().clone(),
            timestamp: Local::now(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn command(&self) -> &Option<String> {
        &self.command
    }

    pub fn source(&self) -> &Option<Source> {
        &self.source
    }

    pub fn timestamp(&self) -> DateTime<Local> {
        self.timestamp
    }
}
