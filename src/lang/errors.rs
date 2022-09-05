use std::error::Error;
use std::fmt::Display;
use crate::lang::ast::location::Location;
use CrushErrorType::*;
use std::cmp::{min, max};

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

#[derive(Debug, PartialEq, Eq)]
struct ErrorLine {
    line_number: usize,
    line: String,
    location: Location,
}

impl ErrorLine {
    fn format_location(&self) -> String {
        format!("{}{}", " ".repeat(self.location.start), "^".repeat(self.location.len()))
    }
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
                let mut res = String::new();
                let lines = extract_location(def, loc);
                if lines.len() == 1 && lines[0].line_number == 1 {
                    res.push_str(&format!("{}\n{}\n", lines[0].line, lines[0].format_location()));
                } else {
                    for line in lines {
                        res.push_str(&format!("Line {}\n{}\n{}\n", line.line_number, line.line, line.format_location()));
                    }
                }
                Some(res)
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

pub fn mandate_argument<T>(result: Option<T>, message: impl Into<String>, location: Location) -> CrushResult<T> {
    match result {
        Some(v) => Ok(v),
        None => Err(CrushError {
            error_type: InvalidData(message.into()),
            location: Some(location),
            definition: None,
        }),
    }
}

fn extract_location(s: &str, loc: Location) -> Vec<ErrorLine> {
    let mut res = Vec::new();

    let mut line = 1;
    let mut start = 0;
    for (off, ch) in s.char_indices().chain(vec![(s.len(), '\n')]) {
        if ch == '\n' {
            if off > loc.start && start < loc.end {
                res.push(
                    ErrorLine {
                        line_number: line,
                        line: s[start..(off)].to_string(),
                        location: Location::new(
                            max(start, loc.start) - start,
                            min(off, loc.end) - start),
                    }
                );
            }
            start = off + 1;
            line += 1;
        }
    }

    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_error_location_formation() {
        let line = ErrorLine {
            line_number: 1,
            line: "remote:exec --hsot=foo".to_string(),
            location: Location::new(12, 22),
        };
        assert_eq!(
            line.format_location(),
            "            ^^^^^^^^^^".to_string()
        );
    }


    #[test]
    fn check_simple_location_extraction() {
        assert_eq!(
            extract_location("remote:exec --hsot=foo", Location::new(12, 22)),
            vec![
                ErrorLine {
                    line_number: 1,
                    line: "remote:exec --hsot=foo".to_string(),
                    location: Location::new(12, 22),
                },
            ]
        );
    }

    #[test]
    fn check_second_line_location_extraction() {
        assert_eq!(
            extract_location("find .\nremote:exec --hsot=foo", Location::new(19, 29)),
            vec![
                ErrorLine {
                    line_number: 2,
                    line: "remote:exec --hsot=foo".to_string(),
                    location: Location::new(12, 22),
                },
            ]
        );
    }

    #[test]
    fn check_first_line_location_extraction() {
        assert_eq!(
            extract_location("remote:exec --hsot=foo\nfind .", Location::new(12, 22)),
            vec![
                ErrorLine {
                    line_number: 1,
                    line: "remote:exec --hsot=foo".to_string(),
                    location: Location::new(12, 22),
                },
            ]
        );
    }

    #[test]
    fn check_whole_line_location_extraction() {
        assert_eq!(
            extract_location("echo 1\necho 2\necho 3", Location::new(7, 13)),
            vec![
                ErrorLine {
                    line_number: 2,
                    line: "echo 2".to_string(),
                    location: Location::new(0, 6),
                },
            ]
        );
    }

    #[test]
    fn check_multi_line_location_extraction() {
        assert_eq!(
            extract_location("echo 1\necho 2\necho 3", Location::new(5, 18)),
            vec![
                ErrorLine {
                    line_number: 1,
                    line: "echo 1".to_string(),
                    location: Location::new(5, 6),
                },
                ErrorLine {
                    line_number: 2,
                    line: "echo 2".to_string(),
                    location: Location::new(0, 6),
                },
                ErrorLine {
                    line_number: 3,
                    line: "echo 3".to_string(),
                    location: Location::new(0, 4),
                },
            ]
        );
    }
}