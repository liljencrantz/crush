/**
The type representing any value in crush.
 */
mod value_definition;
mod value_type;

use std::cmp::Ordering;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono::{DateTime, Local, TimeDelta};
use regex::Regex;

use crate::lang::data::r#struct::Struct;
use crate::lang::data::r#struct::StructReader;
use crate::lang::data::{
    binary::BinaryReader, dict::Dict, dict::DictReader, list::List, table::ColumnType,
    table::TableReader,
};
use crate::lang::errors::{CrushResult, command_error, data_error};
use crate::lang::pipe::{Stream, TableInputStream, TableOutputStream};
use crate::lang::state::scope::Scope;
use crate::util::time::duration_format;
use crate::{lang::data::table::Table, lang::errors::error, util::file::cwd, util::glob::Glob};
use chrono::Duration;

use crate::data::table::ColumnFormat;
use crate::lang::ast::tracked_string::TrackedString;
use crate::lang::command::{Command, CommandBinder};
use crate::lang::help::Help;
use crate::lang::pretty::format_buffer;
use crate::lang::signature::number;
use crate::lang::vec_reader::VecReader;
use crate::state::global_state::FormatData;
use crate::state::scope::ScopeReader;
use crate::util::display_non_recursive::DisplayNonRecursive;
use crate::util::escape::{escape, escape_without_quotes};
use crate::util::identity_arc::Identity;
use crate::util::integer_formater::format_integer;
use crate::util::repr::Repr;
use ordered_map::OrderedMap;
use std::fmt::{Display, Formatter};
use std::io::Read;
use std::ops::Add;
use std::sync::Arc;
pub use value_definition::ValueDefinition;
pub use value_type::ValueType;

pub type BinaryInputStream = Box<dyn BinaryReader + Send + Sync>;

pub enum Value {
    Empty,
    String(Arc<str>),
    Integer(i128),
    Time(DateTime<Local>),
    Duration(Duration),
    Glob(Glob),
    Regex(String, Regex),
    Command(Command),
    TableInputStream(TableInputStream),
    TableOutputStream(TableOutputStream),
    File(Arc<Path>),
    Table(Table),
    Struct(Struct),
    List(List),
    Dict(Dict),
    Scope(Scope),
    Bool(bool),
    Float(f64),
    BinaryInputStream(BinaryInputStream),
    Binary(Arc<[u8]>),
    Type(ValueType),
}

#[derive(Copy, Clone, Debug)]
pub enum ComparisonMode {
    Regular,
    CaseInsensitive,
}

impl DisplayNonRecursive for Value {
    fn fmt_non_recursive(
        &self,
        f: &mut Formatter<'_>,
        seen: &mut HashSet<u64>,
    ) -> std::fmt::Result {
        match self {
            Value::String(val) => std::fmt::Display::fmt(val, f),
            Value::Integer(val) => std::fmt::Display::fmt(val, f),
            Value::Time(val) => f.write_str(&val.format("%Y-%m-%d %H:%M:%S %z").to_string()),
            Value::Glob(val) => std::fmt::Display::fmt(val, f),
            Value::Regex(val, _) => {
                f.write_str("^(")?;
                f.write_str(val)?;
                f.write_str(")")
            }
            Value::File(val) => {
                std::fmt::Display::fmt(val.to_str().unwrap_or("<invalid filename>"), f)
            }
            Value::List(l) => l.fmt_non_recursive(f, seen),
            Value::Duration(d) => f.write_str(&duration_format(d)),
            Value::Scope(env) => env.fmt(f),
            Value::Bool(v) => std::fmt::Display::fmt(if *v { "$true" } else { "$false" }, f),
            Value::Dict(d) => d.fmt_non_recursive(f, seen),
            Value::Float(val) => std::fmt::Display::fmt(val, f),
            Value::Binary(v) => f.write_str(&format_buffer(v, true)),
            Value::Type(t) => std::fmt::Display::fmt(t, f),
            Value::Struct(s) => s.fmt_non_recursive(f, seen),
            Value::Command(cmd) => Display::fmt(cmd, f),
            Value::TableInputStream(_)
            | Value::TableOutputStream(_)
            | Value::Table(_)
            | Value::BinaryInputStream(_)
            | Value::Empty => {
                f.write_str("<")?;
                std::fmt::Display::fmt(&self.value_type(), f)?;
                f.write_str(">")
            }
        }
    }
}

impl Repr for Value {
    fn repr(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::String(val) => f.write_str(escape(val).as_str()),
            Value::Integer(val) => std::fmt::Display::fmt(val, f),
            Value::Time(_) => {
                panic!()
            }
            Value::Glob(val) => std::fmt::Display::fmt(val, f),
            Value::Regex(val, _) => {
                f.write_str("^(")?;
                f.write_str(val)?;
                f.write_str(")")
            }
            Value::File(val) => {
                f.write_str("'")?;
                f.write_str(
                    escape_without_quotes(val.to_str().unwrap_or("<invalid filename>")).as_str(),
                )?;
                f.write_str("'")
            }
            Value::List(_) => panic!(),
            Value::Duration(_) => panic!(),
            Value::Scope(env) => env.fmt(f),
            Value::Bool(v) => std::fmt::Display::fmt(if *v { "$true" } else { "$false" }, f),
            Value::Dict(_) => panic!(),
            Value::Float(val) => std::fmt::Display::fmt(val, f),
            Value::Binary(_) => panic!(),
            Value::Type(t) => std::fmt::Display::fmt(t, f),
            Value::Struct(_) => panic!(),
            Value::Command(cmd) => Display::fmt(cmd, f),
            Value::TableInputStream(_)
            | Value::TableOutputStream(_)
            | Value::Table(_)
            | Value::BinaryInputStream(_) => panic!(),
            Value::Empty => panic!(),
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut seen = HashSet::new();
        self.fmt_non_recursive(f, &mut seen)
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

impl From<Vec<u8>> for Value {
    fn from(s: Vec<u8>) -> Value {
        Value::Binary(Arc::from(s))
    }
}

impl From<&[Box<[u8]>]> for Value {
    fn from(s: &[Box<[u8]>]) -> Value {
        Value::Binary(Arc::from(s.concat()))
    }
}

impl From<&Vec<u8>> for Value {
    fn from(s: &Vec<u8>) -> Value {
        Value::Binary(Arc::from(s.as_ref()))
    }
}

impl From<TimeDelta> for Value {
    fn from(d: TimeDelta) -> Value {
        Value::Duration(d)
    }
}

impl From<&[u8]> for Value {
    fn from(s: &[u8]) -> Value {
        Value::Binary(Arc::from(s))
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

impl From<usize> for Value {
    fn from(v: usize) -> Value {
        Value::Integer(v as i128)
    }
}

impl From<number::Number> for Value {
    fn from(v: number::Number) -> Value {
        match v {
            number::Number::Integer(i) => Value::Integer(i),
            number::Number::Float(f) => Value::Float(f),
        }
    }
}

impl From<u32> for Value {
    fn from(v: u32) -> Value {
        Value::Integer(v as i128)
    }
}

impl From<sysinfo::Uid> for Value {
    fn from(v: sysinfo::Uid) -> Value {
        Value::from(v.add(0))
    }
}

impl From<sysinfo::Gid> for Value {
    fn from(v: sysinfo::Gid) -> Value {
        Value::from(v.add(0))
    }
}

impl From<u64> for Value {
    fn from(v: u64) -> Value {
        Value::Integer(v as i128)
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

impl From<Struct> for Value {
    fn from(v: Struct) -> Value {
        Value::Struct(v)
    }
}

impl From<Command> for Value {
    fn from(v: Command) -> Value {
        Value::Command(Arc::from(v))
    }
}

impl From<PathBuf> for Value {
    fn from(s: PathBuf) -> Value {
        Value::File(Arc::from(s))
    }
}

impl From<&PathBuf> for Value {
    fn from(s: &PathBuf) -> Value {
        Value::File(Arc::from(s.as_path()))
    }
}

impl From<&Path> for Value {
    fn from(s: &Path) -> Value {
        Value::File(Arc::from(s))
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
                    .map(|m| Value::Command(m.clone()))
            }),
            Value::Type(t) => t.fields().get(name).map(|m| Value::Command(m.clone())),
            _ => self
                .value_type()
                .fields()
                .get(name)
                .map(|m| Value::Command(m.clone())),
        })
    }

    pub fn fields(&self) -> Vec<String> {
        let mut res = Vec::new();
        match self {
            Value::Struct(s) => res.append(&mut s.keys()),
            Value::Scope(scope) => {
                res.append(
                    &mut scope
                        .dump_local()
                        .unwrap()
                        .iter()
                        .map(|(k, _)| k.to_string())
                        .collect(),
                );
                add_keys(self.value_type().fields(), &mut res);
            }
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
            2 => Ok(self.field(&path[1])?.ok_or("Invalid path")?),
            _ => self
                .field(&path[1])?
                .ok_or("Invalid path")?
                .get_recursive(&path[1..]),
        }
    }

    pub fn alignment(&self) -> Alignment {
        match self {
            Value::Time(_) | Value::Duration(_) | Value::Integer(_) | Value::Float(_) => {
                Alignment::Right
            }
            _ => Alignment::Left,
        }
    }

    pub fn stream(&self) -> CrushResult<Stream> {
        Ok(match self {
            Value::TableInputStream(s) => Box::from(s.clone()),
            Value::Table(r) => Box::from(TableReader::new(r.clone())),
            Value::List(l) => l.stream(),
            Value::Dict(d) => Box::from(DictReader::new(d.clone())),
            Value::Struct(s) => Box::from(StructReader::new(s.clone())),
            Value::Scope(s) => Box::from(ScopeReader::new(s.clone())),
            Value::Glob(l) => {
                let mut paths = Vec::<PathBuf>::new();
                l.glob_files(&cwd()?, &mut paths)?;
                Box::from(VecReader::new(
                    paths.iter().map(|e| Value::from(e.to_path_buf())).collect(),
                    ValueType::File,
                ))
            }
            v => return data_error(format!("Expected a value that can be streamed, got a value of type `{}`", v.value_type())),
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
            Value::Empty => ValueType::Empty,
            Value::BinaryInputStream(_) => ValueType::BinaryInputStream,
            Value::Binary(_) => ValueType::Binary,
            Value::Type(_) => ValueType::Type,
        }
    }

    pub fn matches(&self, value: &str) -> CrushResult<bool> {
        match self {
            Value::String(s) => Ok(*value == **s),
            Value::Glob(pattern) => Ok(pattern.matches(value)),
            Value::Regex(_, re) => Ok(re.is_match(value)),
            _ => return command_error("Invalid value for match"),
        }
    }

    pub fn materialize(self) -> CrushResult<Value> {
        Ok(match self {
            Value::TableInputStream(output) => {
                let mut rows = Vec::new();
                while let Ok(r) = output.recv() {
                    rows.push(r.materialize()?);
                }
                Value::Table(Table::from((
                    ColumnType::materialize(output.types())?,
                    rows,
                )))
            }
            Value::BinaryInputStream(mut s) => {
                let mut vec = Vec::new();
                std::io::copy(s.as_mut(), &mut vec)?;
                Value::from(vec)
            }
            Value::Table(r) => Value::Table(r.materialize()?),
            Value::Dict(d) => d.materialize()?.into(),
            Value::Struct(r) => Value::Struct(r.materialize()?),
            Value::List(l) => l.materialize()?.into(),
            Value::TableOutputStream(_) => {
                return error("Value of type table_output_stream can't be materialized");
            }
            Value::Empty
            | Value::String(_)
            | Value::Integer(_)
            | Value::Time(_)
            | Value::Duration(_)
            | Value::Glob(_)
            | Value::Regex(_, _)
            | Value::Command(_)
            | Value::File(_)
            | Value::Scope(_)
            | Value::Bool(_)
            | Value::Float(_)
            | Value::Binary(_)
            | Value::Type(_) => self,
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

        let str_val = match self {
            Value::BinaryInputStream(mut reader) => {
                let mut res = String::new();
                reader.read_to_string(&mut res)?;
                res
            }
            v => v.to_string(),
        };

        match new_type {
            ValueType::File => Ok(Value::from(PathBuf::from(str_val.as_str()))),
            ValueType::Glob => Ok(Value::Glob(Glob::new(str_val.as_str()))),
            ValueType::Integer => Ok(str_val.parse::<i128>().map(Value::Integer)?),
            ValueType::Regex => Ok(Regex::new(str_val.as_str()).map(|v| Value::Regex(str_val, v))?),
            ValueType::Binary => Ok(Value::Binary(str_val.bytes().collect())),
            ValueType::Float => Ok(Value::Float(f64::from_str(&str_val)?)),
            ValueType::Bool => Ok(Value::Bool(match str_val.as_str() {
                "true" => true,
                "false" => false,
                _ => {
                    return error(format!("Can't convert value '{}' to boolean", str_val).as_str());
                }
            })),
            ValueType::String => Ok(Value::from(str_val)),
            ValueType::Time => error("invalid convert"),
            ValueType::Duration => Ok(Value::Duration(Duration::seconds(i64::from_str(&str_val)?))),
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
            ValueType::OneOf(_) => error("Can't convert to multiple types"),
        }
    }

    /**
    Format this value in a way appropriate for use in the pretty printer.

    * Escape non-printable strings
    * Respect integer grouping, but use _ instead of whatever number group
      separator the locale prescribes, so that the number can be copied
      and pasted into the terminal again.
     */
    pub fn to_pretty_string(
        &self,
        format_data: &FormatData,
        format: &ColumnFormat,
        table: bool,
    ) -> String {
        match self {
            Value::String(val) => {
                if has_non_printable(val) {
                    escape(val)
                } else {
                    val.to_string()
                }
            }

            Value::Float(f) => match format {
                ColumnFormat::ByteUnit | ColumnFormat::None => {
                    if table {
                        format!("{:.*}", format_data.float_precision(), f)
                    } else {
                        format!("{}", f)
                    }
                }
                ColumnFormat::Percentage => {
                    format!("{:.*}%", format_data.percentage_precision(), f * 100.0)
                }
                ColumnFormat::Temperature => format!(
                    "{:.*} {}",
                    format_data.temperature_precision(),
                    format_data.temperature().format(*f),
                    format_data.temperature().unit()
                ),
            },

            Value::Integer(i) => match format {
                ColumnFormat::Percentage | ColumnFormat::Temperature | ColumnFormat::None => {
                    format_integer(*i, format_data.grouping())
                }
                ColumnFormat::ByteUnit => {
                    format_data.byte_unit().format(*i, format_data.grouping())
                }
            },

            _ => self.to_string(),
        }
    }

    pub fn param_partial_cmp(&self, other: &Value, mode: ComparisonMode) -> Option<Ordering> {
        match (self, other) {
            (Value::String(val1), Value::String(val2)) => match mode {
                ComparisonMode::Regular => Some(val1.cmp(val2)),
                ComparisonMode::CaseInsensitive => {
                    Some(val1.to_lowercase().cmp(&val2.to_lowercase()))
                }
            },
            (Value::Integer(val1), Value::Integer(val2)) => Some(val1.cmp(val2)),
            (Value::Float(val1), Value::Integer(val2)) => val1.partial_cmp(&(*val2 as f64)),
            (Value::Integer(val1), Value::Float(val2)) => (*val1 as f64).partial_cmp(val2),
            (Value::Float(val1), Value::Float(val2)) => val1.partial_cmp(val2),
            (Value::Time(val1), Value::Time(val2)) => Some(val1.cmp(val2)),
            (Value::Duration(val1), Value::Duration(val2)) => Some(val1.cmp(val2)),
            (Value::Glob(val1), Value::Glob(val2)) => Some(val1.cmp(val2)),
            (Value::Regex(val1, _), Value::Regex(val2, _)) => Some(val1.cmp(val2)),
            (Value::File(val1), Value::File(val2)) => match mode {
                ComparisonMode::Regular => Some(val1.cmp(val2)),
                ComparisonMode::CaseInsensitive => Some(
                    val1.to_string_lossy()
                        .to_lowercase()
                        .cmp(&val2.to_string_lossy().to_lowercase()),
                ),
            },
            (Value::Table(val1), Value::Table(val2)) => val1.partial_cmp(val2),
            (Value::Struct(val1), Value::Struct(val2)) => val1.partial_cmp(val2),
            (Value::List(val1), Value::List(val2)) => val1.param_partial_cmp(val2, mode),
            (Value::Dict(val1), Value::Dict(val2)) => val1.partial_cmp(val2),
            (Value::Bool(val1), Value::Bool(val2)) => Some(val1.cmp(val2)),
            (Value::Binary(val1), Value::Binary(val2)) => Some(val1.cmp(val2)),
            _ => None,
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
            Value::Command(v) => Value::Command(v.clone()),
            Value::File(v) => Value::File(v.clone()),
            Value::Table(r) => Value::Table(r.clone()),
            Value::Struct(r) => Value::Struct(r.clone()),
            Value::TableInputStream(s) => Value::TableInputStream(s.clone()),
            Value::TableOutputStream(s) => Value::TableOutputStream(s.clone()),
            Value::List(l) => l.clone().into(),
            Value::Duration(d) => Value::Duration(*d),
            Value::Scope(e) => Value::Scope(e.clone()),
            Value::Bool(v) => Value::Bool(*v),
            Value::Dict(d) => d.clone().into(),
            Value::Float(f) => Value::Float(*f),
            Value::Empty => Value::Empty,
            Value::BinaryInputStream(v) => Value::BinaryInputStream(v.as_ref().clone()),
            Value::Binary(v) => Value::Binary(v.clone()),
            Value::Type(t) => Value::Type(t.clone()),
        }
    }
}

fn integer_decode(val: f64) -> (u64, i16, i8) {
    let bits: u64 = f64::to_bits(val);
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

impl Hash for Value {
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
            Value::File(v) => v.hash(state),
            Value::Duration(d) => d.hash(state),
            Value::Bool(v) => v.hash(state),
            Value::Binary(v) => v.hash(state),
            Value::Struct(_)
            | Value::Command(_)
            | Value::Scope(_)
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
            Value::Empty => {}
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

impl PartialEq for Value {
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
            (Value::Scope(val1), Value::Scope(val2)) => val1.id() == val2.id(),
            (Value::Type(val1), Value::Type(val2)) => val1 == val2,
            _ => false,
        }
    }
}

pub enum Alignment {
    Left,
    Right,
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Value) -> Option<Ordering> {
        self.param_partial_cmp(other, ComparisonMode::Regular)
    }
}

impl Eq for Value {}

impl Help for Value {
    fn signature(&self) -> String {
        match self {
            Value::Scope(s) => s.signature(),
            Value::Command(s) => s.signature(),
            Value::Type(s) => s.signature(),
            Value::Struct(s) => s.signature(),
            v => v.value_type().signature(),
        }
    }

    fn short_help(&self) -> String {
        match self {
            Value::Scope(s) => s.short_help(),
            Value::Command(s) => s.short_help(),
            Value::Type(s) => s.short_help(),
            Value::Struct(s) => s.short_help(),
            v => v.value_type().short_help(),
        }
    }

    fn long_help(&self) -> Option<String> {
        match self {
            Value::Scope(s) => s.long_help(),
            Value::Command(s) => s.long_help(),
            Value::Type(s) => s.long_help(),
            Value::Struct(s) => s.long_help(),
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
        assert_eq!(Value::from("1d").convert(ValueType::Integer).is_err(), true);
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
}
