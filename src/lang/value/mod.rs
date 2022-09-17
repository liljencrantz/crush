/**
The type representing all values in crush.
 */
mod value_definition;
mod value_type;

use std::cmp::Ordering;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono::{DateTime, Local};
use regex::Regex;

use crate::lang::errors::{argument_error_legacy, mandate, CrushResult, eof_error};
use crate::lang::data::r#struct::Struct;
use crate::lang::data::r#struct::StructReader;
use crate::lang::state::scope::Scope;
use crate::lang::pipe::{streams, InputStream, Stream, OutputStream, CrushStream};
use crate::lang::data::{
    binary::BinaryReader, dict::Dict, dict::DictReader, list::List,
    table::ColumnType, table::TableReader,
};
use crate::util::time::duration_format;
use crate::{
    lang::errors::{error, to_crush_error},
    lang::data::table::Table,
    util::file::cwd,
    util::glob::Glob,
};
use chrono::Duration;

use crate::lang::command::Command;
use crate::lang::help::Help;
use crate::lang::pretty::format_buffer;
use crate::lang::printer::Printer;
use crate::util::regex::RegexFileMatcher;
use ordered_map::OrderedMap;
pub use value_definition::ValueDefinition;
pub use value_type::ValueType;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;
use num_format::Grouping;
use crate::data::table::Row;
use crate::lang::ast::tracked_string::TrackedString;
use crate::util::escape::escape;
use crate::util::replace::Replace;

pub enum Value {
    Empty(),
    String(Arc<str>),
    Integer(i128),
    Time(DateTime<Local>),
    Duration(Duration),
    Glob(Glob),
    Regex(String, Regex),
    Command(Command),
    TableInputStream(InputStream),
    TableOutputStream(OutputStream),
    File(PathBuf),
    Table(Table),
    Struct(Struct),
    List(List),
    Dict(Dict),
    Scope(Scope),
    Bool(bool),
    Float(f64),
    BinaryInputStream(Box<dyn BinaryReader + Send + Sync>),
    Binary(Vec<u8>),
    Type(ValueType),
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::String(val) => std::fmt::Display::fmt(val, f),
            Value::Integer(val) => std::fmt::Display::fmt(val, f),
            Value::Time(val) => f.write_str(&val.format("%Y-%m-%d %H:%M:%S %z").to_string()),
            Value::Glob(val) => std::fmt::Display::fmt(val, f),
            Value::Regex(val, _) => {
                f.write_str("re\"")?;
                f.write_str(val)?;
                f.write_str("\"")
            }
            Value::File(val) => std::fmt::Display::fmt(val.to_str().unwrap_or("<invalid filename>"), f),
            Value::List(l) => std::fmt::Display::fmt(l, f),
            Value::Duration(d) => f.write_str(&duration_format(d)),
            Value::Scope(env) => env.fmt(f),
            Value::Bool(v) => std::fmt::Display::fmt(if *v { "true" } else { "false" }, f),
            Value::Dict(d) => d.fmt(f),
            Value::Float(val) => std::fmt::Display::fmt(val, f),
            Value::Binary(v) => f.write_str(&format_buffer(v, true)),
            Value::Type(t) => std::fmt::Display::fmt(t, f),
            Value::Struct(s) => s.fmt(f),
            _ => {
                f.write_str("<")?;
                std::fmt::Display::fmt(&self.value_type(), f)?;
                f.write_str(">")
            }
        }
    }
}

fn add_keys<T>(map: &OrderedMap<String, T>, res: &mut Vec<String>) {
    res.append(&mut map.keys().map(|k| k.to_string()).collect());
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::String(Arc::from(s))
    }
}

impl From<&String> for Value {
    fn from(s: &String) -> Value {
        Value::String(Arc::from(s.as_str()))
    }
}

impl From<&TrackedString> for Value {
    fn from(s: &TrackedString) -> Value {
        Value::String(Arc::from(s.string.as_str()))
    }
}

impl From<TrackedString> for Value {
    fn from(s: TrackedString) -> Value {
        Value::String(Arc::from(s.string))
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::String(Arc::from(s))
    }
}

impl From<char> for Value {
    fn from(v: char) -> Value {
        Value::String(Arc::from(v.to_string()))
    }
}

impl From<i128> for Value {
    fn from(v: i128) -> Value {
        Value::Integer(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Value {
        Value::Integer(v as i128)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Value {
        Value::Float(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Value {
        Value::Bool(v)
    }
}

pub struct VecReader {
    vec: Vec<Value>,
    types: Vec<ColumnType>,
    idx: usize,
}

impl VecReader {
    pub fn new(
        vec: Vec<Value>,
        column_type: ValueType,
    ) -> VecReader {
        VecReader {
            vec,
            types: vec![ColumnType::new("value", column_type)],
            idx: 0,
        }
    }
}

impl CrushStream for VecReader {
    fn read(&mut self) -> CrushResult<Row> {
        self.idx += 1;
        if self.idx > self.vec.len() {
            return eof_error()
        }
        Ok(Row::new(vec![self.vec.replace(self.idx - 1, Value::Empty())]))
    }

    fn read_timeout(
        &mut self,
        _timeout: Duration,
    ) -> Result<Row, crate::lang::pipe::RecvTimeoutError> {
        match self.read() {
            Ok(r) => Ok(r),
            Err(_) => Err(crate::lang::pipe::RecvTimeoutError::Disconnected),
        }
    }

    fn types(&self) -> &[ColumnType] {
        &self.types
    }
}

impl Value {
    pub fn bind(self, this: Value) -> Value {
        match self {
            Value::Command(cmd) => Value::Command(cmd.bind(this)),
            v => v,
        }
    }

    pub fn field(&self, name: &str) -> CrushResult<Option<Value>> {
        Ok(match self {
            Value::Struct(s) => s.get(name),
            Value::Scope(subenv) => subenv.get(name)?.or_else(|| {
                self.value_type()
                    .fields()
                    .get(name)
                    .map(|m| Value::Command(m.as_ref().copy()))
            }),
            Value::Type(t) => t
                .fields()
                .get(name)
                .map(|m| Value::Command(m.as_ref().copy())),
            _ => self
                .value_type()
                .fields()
                .get(name)
                .map(|m| Value::Command(m.as_ref().copy())),
        })
    }

    pub fn fields(&self) -> Vec<String> {
        let mut res = Vec::new();
        match self {
            Value::Struct(s) => res.append(&mut s.keys()),
            Value::Scope(scope) => {
                res.append(&mut scope.dump_local().unwrap().iter().map(|(k, _)| k.to_string()).collect());
                add_keys(self.value_type().fields(), &mut res);
            },
            Value::Type(t) => add_keys(t.fields(), &mut res),
            _ => add_keys(self.value_type().fields(), &mut res),
        }
        res.sort_by(|x, y| x.cmp(y));

        res
    }

    pub fn get_recursive(&self, path: &[String]) -> CrushResult<Value> {
        match path.len() {
            0 => error("Invalid path"),
            1 => Ok(self.clone()),
            2 => mandate(self.field(&path[1])?, "Invalid path"),
            _ => mandate(self.field(&path[1])?, "Invalid path")?.get_recursive(&path[1..]),
        }
    }

    pub fn path(&self, name: &str) -> Option<Value> {
        match self {
            Value::File(s) => Some(Value::File(s.join(name))),
            _ => None,
        }
    }

    pub fn alignment(&self) -> Alignment {
        match self {
            Value::Time(_) | Value::Duration(_) | Value::Integer(_) => Alignment::Right,
            _ => Alignment::Left,
        }
    }

    pub fn empty_table_input_stream() -> Value {
        let (_s, r) = streams(vec![]);
        Value::TableInputStream(r)
    }

    pub fn stream(&self) -> CrushResult<Option<Stream>> {
        Ok(match self {
            Value::TableInputStream(s) => Some(Box::from(s.clone())),
            Value::Table(r) => Some(Box::from(TableReader::new(r.clone()))),
            Value::List(l) => Some(l.stream()),
            Value::Dict(d) => Some(Box::from(DictReader::new(d.clone()))),
            Value::Struct(s) => Some(Box::from(StructReader::new(s.clone()))),
            Value::Glob(l) => {
                let mut paths = Vec::<PathBuf>::new();
                l.glob_files(&cwd()?, &mut paths)?;
                Some(Box::from(VecReader::new(
                    paths.iter().map(|e| { Value::File(e.to_path_buf()) }).collect(),
                    ValueType::File)))
            }
            _ => None,
        })
    }

    pub fn value_type(&self) -> ValueType {
        match self {
            Value::String(_) => ValueType::String,
            Value::Integer(_) => ValueType::Integer,
            Value::Time(_) => ValueType::Time,
            Value::Glob(_) => ValueType::Glob,
            Value::Regex(_, _) => ValueType::Regex,
            Value::Command(_) => ValueType::Command,
            Value::File(_) => ValueType::File,
            Value::TableInputStream(s) => ValueType::TableInputStream(s.types().to_vec()),
            Value::TableOutputStream(s) => ValueType::TableOutputStream(s.types().to_vec()),
            Value::Table(t) => ValueType::Table(t.types().to_vec()),
            Value::Struct(_) => ValueType::Struct,
            Value::List(l) => l.list_type(),
            Value::Duration(_) => ValueType::Duration,
            Value::Scope(_) => ValueType::Scope,
            Value::Bool(_) => ValueType::Bool,
            Value::Dict(d) => d.dict_type(),
            Value::Float(_) => ValueType::Float,
            Value::Empty() => ValueType::Empty,
            Value::BinaryInputStream(_) => ValueType::BinaryInputStream,
            Value::Binary(_) => ValueType::Binary,
            Value::Type(_) => ValueType::Type,
        }
    }

    pub fn file_expand(&self, v: &mut Vec<PathBuf>, printer: &Printer) -> CrushResult<()> {
        match self {
            Value::String(s) => v.push(PathBuf::from(s.to_string())),
            Value::File(p) => v.push(p.clone()),
            Value::Glob(pattern) => pattern.glob_files(&PathBuf::from("."), v)?,
            Value::Regex(_, re) => re.match_files(&cwd()?, v, printer),
            val => match val.stream()? {
                None => return error("Expected a file name"),
                Some(mut s) => {
                    let t = s.types();
                    if t.len() == 1 && t[0].cell_type == ValueType::File {
                        while let Ok(row) = s.read() {
                            if let Value::File(f) = Vec::from(row).remove(0) {
                                v.push(f);
                            }
                        }
                    } else {
                        return argument_error_legacy("Table stream must contain one column of type file");
                    }
                }
            },
        }
        Ok(())
    }

    pub fn matches(&self, value: &str) -> CrushResult<bool> {
        match self {
            Value::String(s) => Ok(*value == **s),
            Value::Glob(pattern) => Ok(pattern.matches(value)),
            Value::Regex(_, re) => Ok(re.is_match(value)),
            _ => return argument_error_legacy("Invalid value for match"),
        }
    }

    pub fn materialize(self) -> CrushResult<Value> {
        Ok(match self {
            Value::TableInputStream(output) => {
                let mut rows = Vec::new();
                while let Ok(r) = output.recv() {
                    rows.push(r.materialize()?);
                }
                Value::Table(Table::new(ColumnType::materialize(output.types())?, rows))
            }
            Value::BinaryInputStream(mut s) => {
                let mut vec = Vec::new();
                to_crush_error(std::io::copy(s.as_mut(), &mut vec))?;
                Value::Binary(vec)
            }
            Value::Table(r) => Value::Table(r.materialize()?),
            Value::Dict(d) => Value::Dict(d.materialize()?),
            Value::Struct(r) => Value::Struct(r.materialize()?),
            Value::List(l) => Value::List(l.materialize()?),
            _ => self,
        })
    }

    pub fn convert(self, new_type: ValueType) -> CrushResult<Value> {
        if self.value_type() == new_type {
            return Ok(self);
        }

        match (&self, &new_type) {
            (Value::Integer(i), ValueType::Bool) => return Ok(Value::Bool(*i != 0)),
            (Value::Float(f), ValueType::Integer) => return Ok(Value::Integer(*f as i128)),
            _ => {}
        }

        let str_val = self.to_string();

        match new_type {
            ValueType::File => Ok(Value::File(PathBuf::from(str_val.as_str()))),
            ValueType::Glob => Ok(Value::Glob(Glob::new(str_val.as_str()))),
            ValueType::Integer => to_crush_error(str_val.parse::<i128>()).map(Value::Integer),
            ValueType::Regex => {
                to_crush_error(Regex::new(str_val.as_str()).map(|v| Value::Regex(str_val, v)))
            }
            ValueType::Binary => Ok(Value::Binary(str_val.bytes().collect())),
            ValueType::Float => Ok(Value::Float(to_crush_error(f64::from_str(&str_val))?)),
            ValueType::Bool => Ok(Value::Bool(match str_val.as_str() {
                "true" => true,
                "false" => false,
                _ => {
                    return error(format!("Can't convert value '{}' to boolean", str_val).as_str());
                }
            })),
            ValueType::String => Ok(Value::from(str_val)),
            ValueType::Time => error("invalid convert"),
            ValueType::Duration => Ok(Value::Duration(Duration::seconds(to_crush_error(
                i64::from_str(&str_val),
            )?))),
            ValueType::Command => error("invalid convert"),
            ValueType::TableInputStream(_) => error("invalid convert"),
            ValueType::TableOutputStream(_) => error("invalid convert"),
            ValueType::Table(_) => error("invalid convert"),
            ValueType::Struct => error("invalid convert"),
            ValueType::List(_) => error("invalid convert"),
            ValueType::Dict(_, _) => error("invalid convert"),
            ValueType::Scope => error("Invalid convert"),
            ValueType::Empty => error("Invalid convert"),
            ValueType::Any => error("Invalid convert"),
            ValueType::BinaryInputStream => error("invalid convert"),
            ValueType::Type => error("invalid convert"),
        }
    }

    /**
    Format this value in a way appropriate for use in the pretty printer.

    * Escape non-printable strings
    * Respect integer grouping, but use _ intead of whatever number group
      separator the locale prescribes, so that the number can be copied
      and pasted into the terminal again.
     */
    pub fn to_pretty_string(&self, grouping: Grouping) -> String {
        match self {
            Value::String(val) =>
                if has_non_printable(val) {
                    escape(val)
                } else {
                    val.to_string()
                },

            Value::Integer(i) => match grouping {
                Grouping::Standard => {
                    let whole = i.to_string();
                    let mut rest = whole.as_str();
                    let mut res = String::new();
                    if *i < 0 {
                        res.push('-');
                        rest = &rest[1..];
                    }
                    loop {
                        if rest.len() <= 3 {
                            break;
                        }
                        let split = ((rest.len() - 1) % 3) + 1;
                        res.push_str(&rest[0..split]);
                        res.push('_');
                        rest = &rest[split..];
                    }
                    res.push_str(rest);
                    res
                }
                Grouping::Indian => {
                    let whole = i.to_string();
                    let mut rest = whole.as_str();
                    let mut res = String::new();
                    if *i < 0 {
                        res.push('-');
                        rest = &rest[1..];
                    }
                    loop {
                        if rest.len() <= 3 {
                            break;
                        }
                        let split = 1 + rest.len() % 2;
                        res.push_str(&rest[0..split]);
                        res.push('_');
                        rest = &rest[split..];
                    }
                    res.push_str(rest);
                    res
                }
                Grouping::Posix => i.to_string(),
            }
            _ => self.to_string(),
        }
    }
}

fn has_non_printable(s: &str) -> bool {
    for c in s.chars() {
        if c < '\x20' {
            return true;
        }
    }
    false
}

impl Clone for Value {
    fn clone(&self) -> Self {
        match self {
            Value::String(v) => Value::String(v.clone()),
            Value::Integer(v) => Value::Integer(*v),
            Value::Time(v) => Value::Time(*v),
            Value::Glob(v) => Value::Glob(v.clone()),
            Value::Regex(v, r) => Value::Regex(v.clone(), r.clone()),
            Value::Command(v) => Value::Command(v.as_ref().copy()),
            Value::File(v) => Value::File(v.clone()),
            Value::Table(r) => Value::Table(r.clone()),
            Value::Struct(r) => Value::Struct(r.clone()),
            Value::TableInputStream(s) => Value::TableInputStream(s.clone()),
            Value::TableOutputStream(s) => Value::TableOutputStream(s.clone()),
            Value::List(l) => Value::List(l.clone()),
            Value::Duration(d) => Value::Duration(*d),
            Value::Scope(e) => Value::Scope(e.clone()),
            Value::Bool(v) => Value::Bool(*v),
            Value::Dict(d) => Value::Dict(d.clone()),
            Value::Float(f) => Value::Float(*f),
            Value::Empty() => Value::Empty(),
            Value::BinaryInputStream(v) => Value::BinaryInputStream(v.as_ref().clone()),
            Value::Binary(v) => Value::Binary(v.clone()),
            Value::Type(t) => Value::Type(t.clone()),
        }
    }
}

fn integer_decode(val: f64) -> (u64, i16, i8) {
    let bits: u64 = unsafe { std::mem::transmute(val) };
    let sign: i8 = if bits >> 63 == 0 { 1 } else { -1 };
    let mut exponent: i16 = ((bits >> 52) & 0x7ff) as i16;
    let mantissa = if exponent == 0 {
        (bits & 0xf_ffff_ffff_ffff) << 1
    } else {
        (bits & 0xf_ffff_ffff_ffff) | 0x10_0000_0000_0000
    };

    exponent -= 1023 + 52;
    (mantissa, exponent, sign)
}

impl std::hash::Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if !self.value_type().is_hashable() {
            panic!("Can't hash mutable cell types!");
        }
        match self {
            Value::String(v) => v.hash(state),
            Value::Integer(v) => v.hash(state),
            Value::Time(v) => v.hash(state),
            Value::Glob(v) => v.hash(state),
            Value::Regex(v, _) => v.hash(state),
            Value::Command(_) => {}
            Value::File(v) => v.hash(state),
            Value::Duration(d) => d.hash(state),
            Value::Bool(v) => v.hash(state),
            Value::Binary(v) => v.hash(state),
            Value::Struct(v) => v.hash(state),
            Value::Scope(_)
            | Value::Dict(_)
            | Value::Table(_)
            | Value::List(_)
            | Value::TableInputStream(_)
            | Value::TableOutputStream(_)
            | Value::BinaryInputStream(_) => panic!("Can't hash output"),
            Value::Float(v) => {
                let (m, x, s) = integer_decode(*v);
                m.hash(state);
                x.hash(state);
                s.hash(state);
            }
            Value::Empty() => {}
            Value::Type(v) => v.to_string().hash(state),
        }
    }
}

fn file_result_compare(f1: &Path, f2: &Path) -> bool {
    match (f1.canonicalize(), f2.canonicalize()) {
        (Ok(p1), Ok(p2)) => p1 == p2,
        _ => false,
    }
}

impl std::cmp::PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::String(val1), Value::String(val2)) => val1 == val2,
            (Value::Integer(val1), Value::Integer(val2)) => val1 == val2,
            (Value::Time(val1), Value::Time(val2)) => val1 == val2,
            (Value::Duration(val1), Value::Duration(val2)) => val1 == val2,
            (Value::Glob(val1), Value::Glob(val2)) => val1 == val2,
            (Value::Regex(val1, _), Value::Regex(val2, _)) => val1 == val2,
            (Value::File(val1), Value::String(val2)) => {
                file_result_compare(&Path::new(&val2.to_string()), val1.as_ref())
            }
            (Value::Table(val1), Value::Table(val2)) => match val1.partial_cmp(val2) {
                None => false,
                Some(o) => o == Ordering::Equal,
            },
            (Value::Struct(val1), Value::Struct(val2)) => val1 == val2,
            (Value::List(val1), Value::List(val2)) => val1 == val2,
            (Value::Dict(val1), Value::Dict(val2)) => val1 == val2,
            (Value::Bool(val1), Value::Bool(val2)) => val1 == val2,
            (Value::Float(val1), Value::Float(val2)) => val1 == val2,
            (Value::Binary(val1), Value::Binary(val2)) => val1 == val2,
            _ => false,
        }
    }
}

pub enum Alignment {
    Left,
    Right,
}

impl std::cmp::PartialOrd for Value {
    fn partial_cmp(&self, other: &Value) -> Option<Ordering> {
        let t1 = self.value_type();
        let t2 = other.value_type();
        if t1 != t2 {
            return None;
        }

        match (self, other) {
            (Value::String(val1), Value::String(val2)) => Some(val1.cmp(val2)),
            (Value::Integer(val1), Value::Integer(val2)) => Some(val1.cmp(val2)),
            (Value::Time(val1), Value::Time(val2)) => Some(val1.cmp(val2)),
            (Value::Duration(val1), Value::Duration(val2)) => Some(val1.cmp(val2)),
            (Value::Glob(val1), Value::Glob(val2)) => Some(val1.cmp(val2)),
            (Value::Regex(val1, _), Value::Regex(val2, _)) => Some(val1.cmp(val2)),
            (Value::File(val1), Value::File(val2)) => Some(val1.cmp(val2)),
            (Value::Table(val1), Value::Table(val2)) => val1.partial_cmp(val2),
            (Value::Struct(val1), Value::Struct(val2)) => val1.partial_cmp(val2),
            (Value::List(val1), Value::List(val2)) => val1.partial_cmp(val2),
            (Value::Dict(val1), Value::Dict(val2)) => val1.partial_cmp(val2),
            (Value::Bool(val1), Value::Bool(val2)) => Some(val1.cmp(val2)),
            (Value::Float(val1), Value::Float(val2)) => val1.partial_cmp(val2),
            (Value::Binary(val1), Value::Binary(val2)) => Some(val1.cmp(val2)),
            _ => None,
        }
    }
}

impl std::cmp::Eq for Value {}

impl Help for Value {
    fn signature(&self) -> String {
        match self {
            Value::Scope(s) => s.signature(),
            Value::Command(s) => s.signature(),
            v => v.value_type().signature(),
        }
    }

    fn short_help(&self) -> String {
        match self {
            Value::Scope(s) => s.short_help(),
            Value::Command(s) => s.short_help(),
            v => v.value_type().short_help(),
        }
    }

    fn long_help(&self) -> Option<String> {
        match self {
            Value::Scope(s) => s.long_help(),
            Value::Command(s) => s.long_help(),
            v => v.value_type().long_help(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_casts() {
        assert_eq!(
            Value::from("112432").convert(ValueType::Integer).is_err(),
            false
        );
        assert_eq!(
            Value::from("1d").convert(ValueType::Integer).is_err(),
            true
        );
        assert_eq!(Value::from("1d").convert(ValueType::Glob).is_err(), false);
        assert_eq!(Value::from("1d").convert(ValueType::File).is_err(), false);
        assert_eq!(Value::from("1d").convert(ValueType::Time).is_err(), true);
    }

    #[test]
    fn test_duration_format() {
        assert_eq!(duration_format(&Duration::microseconds(0)), "0".to_string());
        assert_eq!(
            duration_format(&Duration::microseconds(1)),
            "0.000001".to_string()
        );
        assert_eq!(
            duration_format(&Duration::microseconds(100)),
            "0.0001".to_string()
        );
        assert_eq!(
            duration_format(&Duration::milliseconds(1)),
            "0.001".to_string()
        );
        assert_eq!(
            duration_format(&Duration::milliseconds(1000)),
            "1".to_string()
        );
        assert_eq!(
            duration_format(&Duration::milliseconds(1000 * 61)),
            "1:01".to_string()
        );
        assert_eq!(
            duration_format(&Duration::milliseconds(1000 * 3601)),
            "1:00:01".to_string()
        );
        assert_eq!(
            duration_format(&Duration::milliseconds(1000 * (3600 * 24 * 3 + 1))),
            "3d0:00:01".to_string()
        );
        assert_eq!(
            duration_format(&Duration::milliseconds(1000 * (3600 * 24 * 365 * 10 + 1))),
            "10y0d0:00:01".to_string()
        );
        assert_eq!(
            duration_format(&Duration::milliseconds(
                1000 * (3600 * 24 * 365 * 10 + 1) + 1
            )),
            "10y0d0:00:01".to_string()
        );
    }

    #[test]
    fn test_number_format_standard() {
        assert_eq!(Value::Integer(0).to_pretty_string(Grouping::Standard), "0");
        assert_eq!(Value::Integer(123).to_pretty_string(Grouping::Standard), "123");
        assert_eq!(Value::Integer(-123).to_pretty_string(Grouping::Standard), "-123");
        assert_eq!(Value::Integer(1234).to_pretty_string(Grouping::Standard), "1_234");
        assert_eq!(Value::Integer(-1234).to_pretty_string(Grouping::Standard), "-1_234");
        assert_eq!(Value::Integer(123_456_789).to_pretty_string(Grouping::Standard), "123_456_789");
        assert_eq!(Value::Integer(-123_456_789).to_pretty_string(Grouping::Standard), "-123_456_789");
    }

    #[test]
    fn test_number_format_indian() {
        assert_eq!(Value::Integer(0).to_pretty_string(Grouping::Indian), "0");
        assert_eq!(Value::Integer(123).to_pretty_string(Grouping::Indian), "123");
        assert_eq!(Value::Integer(-123).to_pretty_string(Grouping::Indian), "-123");
        assert_eq!(Value::Integer(1234).to_pretty_string(Grouping::Indian), "1_234");
        assert_eq!(Value::Integer(-1234).to_pretty_string(Grouping::Indian), "-1_234");
        assert_eq!(Value::Integer(123_456_789).to_pretty_string(Grouping::Indian), "12_34_56_789");
        assert_eq!(Value::Integer(-123_456_789).to_pretty_string(Grouping::Indian), "-12_34_56_789");
    }

    #[test]
    fn test_number_format_posix() {
        assert_eq!(Value::Integer(0).to_pretty_string(Grouping::Posix), "0");
        assert_eq!(Value::Integer(123).to_pretty_string(Grouping::Posix), "123");
        assert_eq!(Value::Integer(1234).to_pretty_string(Grouping::Posix), "1234");
        assert_eq!(Value::Integer(123_456_789).to_pretty_string(Grouping::Posix), "123456789");
        assert_eq!(Value::Integer(-123).to_pretty_string(Grouping::Posix), "-123");
        assert_eq!(Value::Integer(-1234).to_pretty_string(Grouping::Posix), "-1234");
        assert_eq!(Value::Integer(-123_456_789).to_pretty_string(Grouping::Posix), "-123456789");
    }
}
