/// Crush error handling type.
///
/// Because Crush has a very large number of builtins, many of which use a third party library
/// that implements its own Error handling, the `CrushErrorType` is insanely large.
/// It doesn't do anything that is weird or unusual, it's just big.
use crate::lang::ast::location::Location;
use crate::lang::ast::token;
use CrushErrorType::*;
use reqwest::header::ToStrError;
use std::cmp::{max, min};

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
    #[cfg(target_os = "linux")]
    DbusError(dbus::Error),
    #[cfg(target_os = "linux")]
    Roxmltree(roxmltree::Error),
    AddrParseError(std::net::AddrParseError),
    ToStrError(ToStrError),
    Message(markdown::message::Message),
    FromHexError(hex::FromHexError),
}

#[derive(Debug)]
pub struct CrushError {
    error_type: CrushErrorType,
    location: Option<Location>,
    definition: Option<String>,
    command: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct ErrorLine {
    line_number: usize,
    line: String,
    location: Location,
}

impl ErrorLine {
    fn format_location(&self) -> String {
        format!(
            "{}{}",
            " ".repeat(self.location.start),
            "^".repeat(self.location.len())
        )
    }
}

impl CrushError {
    pub fn error_type(&self) -> &CrushErrorType {
        &self.error_type
    }

    pub fn message(&self) -> String {
        match &self.error_type {
            InvalidArgument(s) | InvalidData(s) | GenericError(s) => s.clone(),
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
            AddrParseError(e) => e.to_string(),
            ToStrError(e) => e.to_string(),
            Message(m) => m.to_string(),
            FromHexError(e) => e.to_string(),
            #[cfg(target_os = "linux")]
            DbusError(e) => e.message().unwrap_or("").to_string(),
            #[cfg(target_os = "linux")]
            Roxmltree(e) => e.to_string(),
        }
    }

    pub fn location(&self) -> Option<Location> {
        self.location
    }

    pub fn command(&self) -> &Option<String> {
        &self.command
    }

    pub fn with_source(self, source: &Option<(String, Location)>) -> CrushError {
        match source {
            None => self,
            Some((def, loc)) => self.with_location(*loc).with_definition(def),
        }
    }

    pub fn with_definition(self, def: impl Into<String>) -> CrushError {
        CrushError {
            error_type: self.error_type,
            location: self.location,
            definition: Some(def.into()),
            command: self.command,
        }
    }

    pub fn with_location(self, l: Location) -> CrushError {
        let location = if let Some(old) = self.location() {
            if old.len() < l.len() { old } else { l }
        } else {
            l
        };
        CrushError {
            error_type: self.error_type,
            location: Some(location),
            definition: self.definition,
            command: self.command,
        }
    }

    pub fn context(&self) -> Option<String> {
        match (&self.definition, self.location) {
            (Some(def), Some(loc)) => {
                let mut res = String::new();
                let lines = extract_location(def, loc);
                if lines.len() == 1 && lines[0].line_number == 1 {
                    res.push_str(&format!(
                        "{}\n{}\n",
                        lines[0].line,
                        lines[0].format_location()
                    ));
                } else {
                    for line in lines {
                        res.push_str(&format!(
                            "Line {}\n{}\n{}\n",
                            line.line_number,
                            line.line,
                            line.format_location()
                        ));
                    }
                }
                Some(res)
            }
            _ => None,
        }
    }

    pub fn is_eof(&self) -> bool {
        matches!(self.error_type(), EOFError)
    }
}

impl From<CrushErrorType> for CrushError {
    fn from(e: CrushErrorType) -> Self {
        CrushError {
            error_type: e,
            location: None,
            definition: None,
            command: None,
        }
    }
}

impl From<std::io::Error> for CrushError {
    fn from(e: std::io::Error) -> Self {
        IOError(e).into()
    }
}

impl From<regex::Error> for CrushError {
    fn from(e: regex::Error) -> Self {
        RegexError(e).into()
    }
}

impl From<hex::FromHexError> for CrushError {
    fn from(e: hex::FromHexError) -> Self {
        FromHexError(e).into()
    }
}

impl From<std::num::ParseIntError> for CrushError {
    fn from(e: std::num::ParseIntError) -> Self {
        ParseIntError(e).into()
    }
}

impl From<std::num::ParseFloatError> for CrushError {
    fn from(e: std::num::ParseFloatError) -> Self {
        ParseFloatError(e).into()
    }
}

impl From<std::net::AddrParseError> for CrushError {
    fn from(e: std::net::AddrParseError) -> Self {
        AddrParseError(e).into()
    }
}

impl From<ToStrError> for CrushError {
    fn from(e: ToStrError) -> Self {
        ToStrError(e).into()
    }
}

#[cfg(target_os = "linux")]
impl From<dbus::Error> for CrushError {
    fn from(e: dbus::Error) -> Self {
        DbusError(e).into()
    }
}

#[cfg(target_os = "linux")]
impl From<roxmltree::Error> for CrushError {
    fn from(e: roxmltree::Error) -> Self {
        Roxmltree(e).into()
    }
}

impl From<crate::lang::ast::lexer::LexicalError> for CrushError {
    fn from(e: crate::lang::ast::lexer::LexicalError) -> Self {
        LexicalError(e).into()
    }
}

impl From<lalrpop_util::ParseError<usize, token::Token<'_>, crate::lang::ast::lexer::LexicalError>>
for CrushError
{
    fn from(
        e: lalrpop_util::ParseError<usize, token::Token, crate::lang::ast::lexer::LexicalError>,
    ) -> Self {
        let location = match e {
            lalrpop_util::ParseError::InvalidToken { location } => {
                Some(Location::new(location, location))
            }
            lalrpop_util::ParseError::UnrecognizedEof { location, .. } => {
                Some(Location::new(location, location))
            }
            lalrpop_util::ParseError::UnrecognizedToken { token, .. } => {
                Some(Location::new(token.0, token.2))
            }
            lalrpop_util::ParseError::ExtraToken { token } => Some(Location::new(token.0, token.2)),
            lalrpop_util::ParseError::User { .. } => None,
        };
        CrushError {
            error_type: ParseError(e.to_string()),
            location,
            definition: None,
            command: None,
        }
    }
}

impl<T> From<crossbeam::channel::SendError<T>> for CrushError {
    fn from(e: crossbeam::channel::SendError<T>) -> Self {
        SendError(e.to_string()).into()
    }
}

impl From<crossbeam::channel::RecvError> for CrushError {
    fn from(e: crossbeam::channel::RecvError) -> Self {
        RecvError(e).into()
    }
}

impl From<num_format::Error> for CrushError {
    fn from(e: num_format::Error) -> Self {
        NumFormatError(e).into()
    }
}

impl<T> From<std::sync::PoisonError<T>> for CrushError {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        PoisonError(e.to_string()).into()
    }
}

impl From<std::num::TryFromIntError> for CrushError {
    fn from(e: std::num::TryFromIntError) -> Self {
        TryFromIntError(e).into()
    }
}

impl From<std::str::ParseBoolError> for CrushError {
    fn from(e: std::str::ParseBoolError) -> Self {
        ParseBoolError(e).into()
    }
}

impl From<rustyline::error::ReadlineError> for CrushError {
    fn from(e: rustyline::error::ReadlineError) -> Self {
        ReadlineError(e).into()
    }
}

impl From<std::string::FromUtf8Error> for CrushError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        FromUtf8Error(e).into()
    }
}

impl From<chrono::OutOfRangeError> for CrushError {
    fn from(e: chrono::OutOfRangeError) -> Self {
        OutOfRangeError(e).into()
    }
}

impl From<std::env::VarError> for CrushError {
    fn from(e: std::env::VarError) -> Self {
        VarError(e).into()
    }
}

impl From<resolv_conf::ParseError> for CrushError {
    fn from(e: resolv_conf::ParseError) -> Self {
        ResolveConfParseError(e).into()
    }
}

impl From<trust_dns_client::proto::error::ProtoError> for CrushError {
    fn from(e: trust_dns_client::proto::error::ProtoError) -> Self {
        DnsProtoError(e).into()
    }
}

impl From<trust_dns_client::error::ClientError> for CrushError {
    fn from(e: trust_dns_client::error::ClientError) -> Self {
        DnsClientError(e).into()
    }
}

impl From<mountpoints::Error> for CrushError {
    fn from(e: mountpoints::Error) -> Self {
        MountpointsError(e).into()
    }
}

impl From<battery::Error> for CrushError {
    fn from(e: battery::Error) -> Self {
        BatteryError(e).into()
    }
}

impl From<nix::errno::Errno> for CrushError {
    fn from(e: nix::errno::Errno) -> Self {
        NixError(e).into()
    }
}

impl From<reqwest::Error> for CrushError {
    fn from(e: reqwest::Error) -> Self {
        ReqwestError(e).into()
    }
}

impl From<std::str::Utf8Error> for CrushError {
    fn from(e: std::str::Utf8Error) -> Self {
        Utf8Error(e).into()
    }
}

impl From<serde_json::Error> for CrushError {
    fn from(e: serde_json::Error) -> Self {
        SerdeJsonError(e).into()
    }
}

impl From<toml::de::Error> for CrushError {
    fn from(e: toml::de::Error) -> Self {
        SerdeTomlError(e).into()
    }
}

impl From<serde_yaml::Error> for CrushError {
    fn from(e: serde_yaml::Error) -> Self {
        SerdeYamlError(e).into()
    }
}

impl From<ssh2::Error> for CrushError {
    fn from(e: ssh2::Error) -> Self {
        SSH2Error(e).into()
    }
}

impl From<chrono::ParseError> for CrushError {
    fn from(e: chrono::ParseError) -> Self {
        ChronoParseError(e).into()
    }
}

impl From<std::char::CharTryFromError> for CrushError {
    fn from(e: std::char::CharTryFromError) -> Self {
        CharTryFromError(e).into()
    }
}

impl From<&str> for CrushError {
    fn from(e: &str) -> Self {
        InvalidData(e.to_string()).into()
    }
}

impl From<String> for CrushError {
    fn from(e: String) -> Self {
        InvalidData(e).into()
    }
}

impl From<&String> for CrushError {
    fn from(e: &String) -> Self {
        InvalidData(e.to_string()).into()
    }
}

impl From<markdown::message::Message> for CrushError {
    fn from(m: markdown::message::Message) -> Self {
        Message(m).into()
    }
}

pub type CrushResult<T> = Result<T, CrushError>;

pub fn eof_error<T>() -> CrushResult<T> {
    Err(EOFError.into())
}

pub fn argument_error_legacy<T>(message: impl Into<String>) -> CrushResult<T> {
    Err(InvalidArgument(message.into()).into())
}

pub fn serialization_error<T>(message: impl Into<String>) -> CrushResult<T> {
    Err(SerializationError(message.into()).into())
}

pub fn argument_error<T>(message: impl Into<String>, location: Location) -> CrushResult<T> {
    Err(CrushError::from(InvalidArgument(message.into())).with_location(location))
}

pub fn data_error<T>(message: impl Into<String>) -> CrushResult<T> {
    Err(InvalidData(message.into()).into())
}

pub fn invalid_jump<T>(message: impl Into<String>) -> CrushResult<T> {
    Err(InvalidJump(message.into()).into())
}

pub fn login_error<T>(message: impl Into<String>) -> CrushResult<T> {
    Err(LoginsError(message.into()).into())
}

pub fn byte_unit_error<T>(s: &str) -> CrushResult<T> {
    Err(ByteUnitError(format!("Unknown byte unit {}", s)).into())
}

pub fn error<T>(message: impl Into<String>) -> CrushResult<T> {
    Err(GenericError(message.into()).into())
}

pub fn mandate_argument<T>(
    result: Option<T>,
    message: impl Into<String>,
    location: Location,
) -> CrushResult<T> {
    match result {
        Some(v) => Ok(v),
        None => Err(CrushError {
            error_type: InvalidData(message.into()),
            location: Some(location),
            definition: None,
            command: None,
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
                res.push(ErrorLine {
                    line_number: line,
                    line: s[start..(off)].to_string(),
                    location: Location::new(
                        max(start, loc.start) - start,
                        min(off, loc.end) - start,
                    ),
                });
            }
            start = off + 1;
            line += 1;
        }
    }

    res
}

pub trait WithCommand {
    fn with_command(self, cmd: impl Into<String>) -> Self;
}

impl<V> WithCommand for CrushResult<V> {
    fn with_command(self, cmd: impl Into<String>) -> CrushResult<V> {
        match self {
            Ok(_) => self,
            Err(err) => Err(CrushError {
                error_type: err.error_type,
                location: err.location,
                definition: err.definition,
                command: Some(cmd.into()),
            }),
        }
    }
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
        assert_eq!(line.format_location(), "            ^^^^^^^^^^".to_string());
    }

    #[test]
    fn check_simple_location_extraction() {
        assert_eq!(
            extract_location("remote:exec --hsot=foo", Location::new(12, 22)),
            vec![ErrorLine {
                line_number: 1,
                line: "remote:exec --hsot=foo".to_string(),
                location: Location::new(12, 22),
            }, ]
        );
    }

    #[test]
    fn check_second_line_location_extraction() {
        assert_eq!(
            extract_location("find .\nremote:exec --hsot=foo", Location::new(19, 29)),
            vec![ErrorLine {
                line_number: 2,
                line: "remote:exec --hsot=foo".to_string(),
                location: Location::new(12, 22),
            }, ]
        );
    }

    #[test]
    fn check_first_line_location_extraction() {
        assert_eq!(
            extract_location("remote:exec --hsot=foo\nfind .", Location::new(12, 22)),
            vec![ErrorLine {
                line_number: 1,
                line: "remote:exec --hsot=foo".to_string(),
                location: Location::new(12, 22),
            }, ]
        );
    }

    #[test]
    fn check_whole_line_location_extraction() {
        assert_eq!(
            extract_location("echo 1\necho 2\necho 3", Location::new(7, 13)),
            vec![ErrorLine {
                line_number: 2,
                line: "echo 2".to_string(),
                location: Location::new(0, 6),
            }, ]
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
