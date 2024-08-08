use crate::lang::ast::location::Location;
use CrushErrorType::*;
use std::cmp::{min, max};
use crate::lang::ast::token;

#[derive(Debug)]
pub enum CrushErrorType {
    InvalidArgument(String),
    InvalidData(String),
    GenericError(String),
    SendError(String),
    RecvError(crossbeam::channel::RecvError),
    EOFError,
    IOError(std::io::Error),
    RegexError(regex::Error),
    ParseIntError(std::num::ParseIntError),
    ParseFloatError(std::num::ParseFloatError),
    ParseBoolError(std::str::ParseBoolError),
    LexicalError(crate::lang::ast::lexer::LexicalError),
    ParseError(String),
    NumFormatError(num_format::Error),
    PoisonError(String),
    TryFromIntError(std::num::TryFromIntError),
    ReadlineError(rustyline::error::ReadlineError),
    FromUtf8Error(std::string::FromUtf8Error),
    OutOfRangeError(chrono::OutOfRangeError),
    VarError(std::env::VarError),
    ByteUnitError(String),
    ResolveConfParseError(resolv_conf::ParseError),
    DnsProtoError(trust_dns_client::proto::error::ProtoError),
    DnsClientError(trust_dns_client::error::ClientError),
    MountpointsError(mountpoints::Error),
    PsutilProcessError(psutil::process::ProcessError),
    SysInfoError(sys_info::Error),
    BatteryError(battery::Error),
    NixError(nix::errno::Errno),
    ReqwestError(reqwest::Error),
    Utf8Error(std::str::Utf8Error),
    SerdeJsonError(serde_json::Error),
    SerdeTomlError(toml::de::Error),
    SerdeYamlError(serde_yaml::Error),
    SSH2Error(ssh2::Error),
    ChronoParseError(chrono::ParseError),
    LoginsError(String),
    CharTryFromError(std::char::CharTryFromError),
    SerializationError(String),
    InvalidJump(String),
}

#[derive(Debug)]
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
    pub fn error_type(&self) -> &CrushErrorType {
        &self.error_type
    }

    pub fn message(&self) -> String {
        match &self.error_type {
            InvalidArgument(s)
            | InvalidData(s)
            | GenericError(s) => s.clone(),
            SendError(e) => e.to_string(),
            EOFError => "EOF error".to_string(),
            IOError(e) => e.to_string(),
            RegexError(e) => e.to_string(),
            ParseIntError(e) => e.to_string(),
            ParseFloatError(e) => e.to_string(),
            LexicalError(e) => e.to_string(),
            ParseError(e) => e.to_string(),
            RecvError(e) => e.to_string(),
            NumFormatError(e) => e.to_string(),
            PoisonError(e) => e.to_string(),
            ParseBoolError(e) => e.to_string(),
            TryFromIntError(e) => e.to_string(),
            ReadlineError(e) => e.to_string(),
            FromUtf8Error(e) => e.to_string(),
            OutOfRangeError(e) => e.to_string(),
            VarError(e) => e.to_string(),
            ByteUnitError(e) => e.to_string(),
            ResolveConfParseError(e) => e.to_string(),
            DnsProtoError(e) => e.to_string(),
            DnsClientError(e) => e.to_string(),
            MountpointsError(e) => e.to_string(),
            PsutilProcessError(e) => e.to_string(),
            SysInfoError(e) => e.to_string(),
            BatteryError(e) => e.to_string(),
            NixError(e) => e.to_string(),
            ReqwestError(e) => e.to_string(),
            Utf8Error(e) => e.to_string(),
            SerdeJsonError(e) => e.to_string(),
            SerdeTomlError(e) => e.to_string(),
            SerdeYamlError(e) => e.to_string(),
            SSH2Error(e) => e.to_string(),
            ChronoParseError(e) => e.to_string(),
            LoginsError(e) => e.to_string(),
            CharTryFromError(e) => e.to_string(),
            SerializationError(e) => e.to_string(),
            InvalidJump(e) => e.to_string(),
        }
    }

    pub fn location(&self) -> Option<Location> {
        self.location
    }

    pub fn with_source(self, source: &Option<(String, Location)>) -> CrushError {
        match source {
            None => self,
            Some((def, loc)) =>
                self.with_location(*loc).with_definition(def),
        }
    }

    pub fn with_definition(self, def: impl Into<String>) -> CrushError {
        CrushError {
            error_type: self.error_type,
            location: self.location,
            definition: Some(def.into()),
        }
    }

    pub fn with_location(self, l: Location) -> CrushError {
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
            error_type: self.error_type,
            location: Some(location),
            definition: self.definition,
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

impl From<std::io::Error> for CrushError {
    fn from(e: std::io::Error) -> Self {
        CrushError {
            error_type: IOError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<regex::Error> for CrushError {
    fn from(e: regex::Error) -> Self {
        CrushError {
            error_type: RegexError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<std::num::ParseIntError> for CrushError {
    fn from(e: std::num::ParseIntError) -> Self {
        CrushError {
            error_type: ParseIntError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<std::num::ParseFloatError> for CrushError {
    fn from(e: std::num::ParseFloatError) -> Self {
        CrushError {
            error_type: ParseFloatError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<crate::lang::ast::lexer::LexicalError> for CrushError {
    fn from(e: crate::lang::ast::lexer::LexicalError) -> Self {
        CrushError {
            error_type: LexicalError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<lalrpop_util::ParseError<usize, token::Token<'_>, crate::lang::ast::lexer::LexicalError>> for CrushError {
    fn from(e: lalrpop_util::ParseError<usize, token::Token, crate::lang::ast::lexer::LexicalError>) -> Self {
        CrushError {
            error_type: ParseError(e.to_string()),
            location: None,
            definition: None,
        }
    }
}

impl<T> From<crossbeam::channel::SendError<T>> for CrushError {
    fn from(e: crossbeam::channel::SendError<T>) -> Self {
        CrushError {
            error_type: SendError(e.to_string()),
            location: None,
            definition: None,
        }
    }
}

impl From<crossbeam::channel::RecvError> for CrushError {
    fn from(e: crossbeam::channel::RecvError) -> Self {
        CrushError {
            error_type: RecvError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<num_format::Error> for CrushError {
    fn from(e: num_format::Error) -> Self {
        CrushError {
            error_type: NumFormatError(e),
            location: None,
            definition: None,
        }
    }
}

impl<T> From<std::sync::PoisonError<T>> for CrushError {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        CrushError {
            error_type: PoisonError(e.to_string()),
            location: None,
            definition: None,
        }
    }
}

impl From<std::num::TryFromIntError> for CrushError {
    fn from(e: std::num::TryFromIntError) -> Self {
        CrushError {
            error_type: TryFromIntError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<std::str::ParseBoolError> for CrushError {
    fn from(e: std::str::ParseBoolError) -> Self {
        CrushError {
            error_type: ParseBoolError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<rustyline::error::ReadlineError> for CrushError {
    fn from(e: rustyline::error::ReadlineError) -> Self {
        CrushError {
            error_type: ReadlineError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<std::string::FromUtf8Error> for CrushError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        CrushError {
            error_type: FromUtf8Error(e),
            location: None,
            definition: None,
        }
    }
}

impl From<chrono::OutOfRangeError> for CrushError {
    fn from(e: chrono::OutOfRangeError) -> Self {
        CrushError {
            error_type: OutOfRangeError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<std::env::VarError> for CrushError {
    fn from(e: std::env::VarError) -> Self {
        CrushError {
            error_type: VarError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<resolv_conf::ParseError> for CrushError {
    fn from(e: resolv_conf::ParseError) -> Self {
        CrushError {
            error_type: ResolveConfParseError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<trust_dns_client::proto::error::ProtoError> for CrushError {
    fn from(e: trust_dns_client::proto::error::ProtoError) -> Self {
        CrushError {
            error_type: DnsProtoError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<trust_dns_client::error::ClientError> for CrushError {
    fn from(e: trust_dns_client::error::ClientError) -> Self {
        CrushError {
            error_type: DnsClientError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<mountpoints::Error> for CrushError {
    fn from(e: mountpoints::Error) -> Self {
        CrushError {
            error_type: MountpointsError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<psutil::process::ProcessError> for CrushError {
    fn from(e: psutil::process::ProcessError) -> Self {
        CrushError {
            error_type: PsutilProcessError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<sys_info::Error> for CrushError {
    fn from(e: sys_info::Error) -> Self {
        CrushError {
            error_type: SysInfoError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<battery::Error> for CrushError {
    fn from(e: battery::Error) -> Self {
        CrushError {
            error_type: BatteryError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<nix::errno::Errno> for CrushError {
    fn from(e: nix::errno::Errno) -> Self {
        CrushError {
            error_type: NixError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<reqwest::Error> for CrushError {
    fn from(e: reqwest::Error) -> Self {
        CrushError {
            error_type: ReqwestError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<std::str::Utf8Error> for CrushError {
    fn from(e: std::str::Utf8Error) -> Self {
        CrushError {
            error_type: Utf8Error(e),
            location: None,
            definition: None,
        }
    }
}

impl From<serde_json::Error> for CrushError {
    fn from(e: serde_json::Error) -> Self {
        CrushError {
            error_type: SerdeJsonError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<toml::de::Error> for CrushError {
    fn from(e: toml::de::Error) -> Self {
        CrushError {
            error_type: SerdeTomlError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<serde_yaml::Error> for CrushError {
    fn from(e: serde_yaml::Error) -> Self {
        CrushError {
            error_type: SerdeYamlError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<ssh2::Error> for CrushError {
    fn from(e: ssh2::Error) -> Self {
        CrushError {
            error_type: SSH2Error(e),
            location: None,
            definition: None,
        }
    }
}

impl From<chrono::ParseError> for CrushError {
    fn from(e: chrono::ParseError) -> Self {
        CrushError {
            error_type: ChronoParseError(e),
            location: None,
            definition: None,
        }
    }
}

impl From<std::char::CharTryFromError> for CrushError {
    fn from(e: std::char::CharTryFromError) -> Self {
        CrushError {
            error_type: CharTryFromError(e),
            location: None,
            definition: None,
        }
    }
}

pub type CrushResult<T> = Result<T, CrushError>;

pub fn eof_error<T>() -> CrushResult<T> {
    Err(CrushError {
        error_type: EOFError,
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

pub fn serialization_error<T>(message: impl Into<String>) -> CrushResult<T> {
    Err(CrushError {
        error_type: SerializationError(message.into()),
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

pub fn invalid_jump<T>(message: impl Into<String>) -> CrushResult<T> {
    Err(CrushError {
        error_type: InvalidJump(message.into()),
        location: None,
        definition: None,
    })
}

pub fn login_error<T>(message: impl Into<String>) -> CrushResult<T>{
    Err(CrushError {
        error_type: LoginsError(message.into()),
        location: None,
        definition: None,
    })
}

pub fn byte_unit_error<T>(s: &str) -> CrushResult<T>{
    Err(CrushError {
        error_type: ByteUnitError(format!("Unknown byte unit {}", s)),
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