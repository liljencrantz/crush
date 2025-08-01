use crate::CrushResult;
use crate::lang::errors::data_error;
use crate::util::hex::from_hex;
use std::convert::TryFrom;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

pub fn escape_without_quotes(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\"' => res += "\\\"",
            '\'' => res += "\\\'",
            '\n' => res += "\\n",
            '\r' => res += "\\r",
            '\t' => res += "\\t",
            '\x1b' => res += "\\e",
            _ => {
                if c < '\x20' {
                    res.push_str(&format!("\\x{:02x}", u32::from(c)));
                } else {
                    res.push(c);
                }
            }
        }
    }
    res
}

pub fn escape(s: &str) -> String {
    let mut res = "\"".to_string();
    res += &escape_without_quotes(s);
    res += "\"";
    res
}

#[derive(Eq, PartialEq)]
enum State {
    Normal,
    Backslash,
    Hex(String),
    Unicode2(String),
    Unicode4(String),
}

pub fn unescape(s: &str) -> CrushResult<String> {
    use State::*;

    let mut res = "".to_string();
    let mut state = Normal;
    for c in s[1..s.len() - 1].chars() {
        match state {
            Backslash => {
                state = Normal;
                match c {
                    'n' => res += "\n",
                    'r' => res += "\r",
                    't' => res += "\t",
                    'e' => res += "\x1b",
                    'x' => state = Hex(String::with_capacity(2)),
                    'u' => state = Unicode2(String::with_capacity(4)),
                    'U' => state = Unicode4(String::with_capacity(8)),
                    _ => res.push(c),
                }
            }

            Normal => {
                if c == '\\' {
                    state = Backslash;
                } else {
                    res += &c.to_string();
                }
            }

            Hex(mut v) => {
                v.push(c);
                if v.len() < 2 {
                    state = Hex(v)
                } else {
                    let bytes = from_hex(&v)?;
                    let chunk = String::from_utf8(bytes)?;
                    res += &chunk;
                    state = Normal
                }
            }

            Unicode2(mut v) => {
                v.push(c);
                if v.len() < 4 {
                    state = Unicode2(v)
                } else {
                    let bytes = from_hex(&v)?;
                    let cc = char::try_from((bytes[0] as u32) << 8 | (bytes[1] as u32))?;
                    res.push(cc);
                    state = Normal
                }
            }

            Unicode4(mut v) => {
                v.push(c);
                if v.len() < 8 {
                    state = Unicode4(v)
                } else {
                    let bytes = from_hex(&v)?;
                    let cc = char::try_from(
                        (bytes[0] as u32) << 24
                            | (bytes[1] as u32) << 16
                            | (bytes[2] as u32) << 8
                            | (bytes[3] as u32) << 0,
                    )?;
                    res.push(cc);
                    state = Normal
                }
            }
        }
    }
    if state != Normal {
        return data_error("Premature end of string");
    }
    Ok(res)
}

pub fn unescape_file(s: &str) -> CrushResult<PathBuf> {
    use State::*;
    let mut res = Vec::new();
    let mut state = Normal;

    for c in s[1..s.len() - 1].chars() {
        match state {
            Backslash => {
                state = Normal;
                match c {
                    'n' => res.push('\n' as u8),
                    'r' => res.push('\r' as u8),
                    't' => res.push('\t' as u8),
                    'e' => res.push('\x1b' as u8),
                    'x' => state = Hex(String::with_capacity(2)),
                    'u' => state = Unicode2(String::with_capacity(4)),
                    'U' => state = Unicode4(String::with_capacity(8)),
                    _ => {
                        let mut b = [0; 4];
                        let len = c.encode_utf8(&mut b).len();
                        res.extend_from_slice(&b[0..len]);
                    }
                }
            }

            Normal => {
                if c == '\\' {
                    state = Backslash;
                } else {
                    let mut b = [0; 4];
                    let len = c.encode_utf8(&mut b).len();
                    res.extend_from_slice(&b[0..len]);
                }
            }

            Hex(mut v) => {
                v.push(c);
                if v.len() < 2 {
                    state = Hex(v)
                } else {
                    let bytes = from_hex(&v)?;
                    res.extend_from_slice(&bytes);
                    state = Normal
                }
            }

            Unicode2(mut v) => {
                v.push(c);
                if v.len() < 4 {
                    state = Unicode2(v)
                } else {
                    let bytes = from_hex(&v)?;
                    res.extend_from_slice(&bytes);
                    state = Normal
                }
            }

            Unicode4(mut v) => {
                v.push(c);
                if v.len() < 8 {
                    state = Unicode4(v)
                } else {
                    let bytes = from_hex(&v)?;
                    res.extend_from_slice(&bytes);
                    state = Normal
                }
            }
        }
    }
    if state != Normal {
        return data_error("Premature end of string");
    }
    Ok(OsString::from_vec(res).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_check() {
        assert_eq!(escape_without_quotes(""), "");
        assert_eq!(escape_without_quotes("a"), "a");
        assert_eq!(escape_without_quotes(r#"""#), r#"\""#);
        assert_eq!(escape_without_quotes("'"), r#"\'"#);
        assert_eq!(escape_without_quotes("\r"), r#"\r"#);
        assert_eq!(escape_without_quotes("\x07"), r#"\x07"#);
        assert_eq!(escape_without_quotes("\x19"), r#"\x19"#);
    }

    #[test]
    fn unescape_check() {
        assert_eq!(unescape(r#""""#).unwrap(), "");
        assert_eq!(unescape(r#""\n\r\t\e""#).unwrap(), "\n\r\t\x1b");
        assert_eq!(unescape(r#""\x01""#).unwrap(), "\x01");
        assert_eq!(unescape(r#""\x0A""#).unwrap(), "\x0a");
        assert_eq!(unescape(r#""\x0a""#).unwrap(), "\x0a");
        assert_eq!(unescape(r#""\x0g""#).is_err(), true);
        assert_eq!(unescape(r#""\x0""#).is_err(), true);
        assert_eq!(unescape(r#""\u72D0""#).unwrap(), "狐");
        assert_eq!(unescape(r#""\U0001F98A""#).unwrap(), "🦊");
    }
}
