use crate::builtins::types;
use crate::lang::command::OutputType::Known;
/// All the different types a value can have.
use crate::lang::command::{Command, OutputType};
use crate::lang::errors::{CrushResult, command_error, error};
use crate::lang::help::Help;
use crate::lang::{data::table::ColumnType, value::Value};
use crate::util::glob::Glob;
use itertools::Itertools;
use ordered_map::OrderedMap;
use regex::Regex;
use std::fmt::{Display, Formatter};
use std::sync::OnceLock;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum ValueType {
    String,
    Integer,
    Time,
    Duration,
    Glob,
    Regex,
    Command,
    File,
    TableInputStream(Vec<ColumnType>),
    TableOutputStream(Vec<ColumnType>),
    Table(Vec<ColumnType>),
    Struct,
    List(Box<ValueType>),
    Dict(Box<ValueType>, Box<ValueType>),
    Scope,
    Bool,
    Float,
    Empty,
    Any,
    BinaryInputStream,
    Binary,
    Type,
    OneOf(Vec<ValueType>),
}

pub fn empty_methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| OrderedMap::new())
}

impl ValueType {
    pub fn table_input_stream(columns: &[ColumnType]) -> ValueType {
        ValueType::TableInputStream(columns.to_vec())
    }

    pub fn output_type(&self) -> OutputType {
        Known(self.clone())
    }

    pub fn one_of(options: Vec<ValueType>) -> ValueType {
        let mut res = Vec::new();
        for vt in options {
            match vt {
                ValueType::Any => return ValueType::Any,
                ValueType::OneOf(mut vt) => res.append(&mut vt),
                _ => res.push(vt),
            }
        }
        ValueType::Any
    }

    pub fn fields(&self) -> &OrderedMap<String, Command> {
        match self {
            ValueType::List(_) => &types::list::methods(),
            ValueType::Dict(_, _) => &types::dict::methods(),
            ValueType::String => &types::string::methods(),
            ValueType::File => &types::file::methods(),
            ValueType::Regex => &types::re::methods(),
            ValueType::Glob => &types::glob::methods(),
            ValueType::Integer => &types::integer::methods(),
            ValueType::Float => &types::float::methods(),
            ValueType::Duration => &types::duration::methods(),
            ValueType::Time => &types::time::methods(),
            ValueType::Table(_) => &types::table::methods(),
            ValueType::TableInputStream(_) => &types::table_input_stream::methods(),
            ValueType::TableOutputStream(_) => &types::table_output_stream::methods(),
            ValueType::Binary => &types::binary::methods(),
            ValueType::Scope => &types::scope::methods(),
            ValueType::Struct => &types::r#struct::methods(),
            ValueType::OneOf(_) => &types::one_of::methods(),
            _ => empty_methods(),
        }
    }

    pub fn is(&self, value: &Value) -> bool {
        self.is_compatible_with(&value.value_type())
    }

    pub fn is_compatible_with(&self, pattern: &ValueType) -> bool {
        match self {
            ValueType::Any => true,
            ValueType::OneOf(types) => types.iter().any(|t| t.is_compatible_with(pattern)),
            _ => self == pattern,
        }
    }

    pub fn materialize(&self) -> CrushResult<ValueType> {
        Ok(match self {
            ValueType::String
            | ValueType::Integer
            | ValueType::Time
            | ValueType::Duration
            | ValueType::Glob
            | ValueType::Regex
            | ValueType::Command
            | ValueType::File
            | ValueType::Scope
            | ValueType::Float
            | ValueType::Empty
            | ValueType::Any
            | ValueType::Binary
            | ValueType::Type
            | ValueType::Struct
            | ValueType::Bool => self.clone(),
            ValueType::BinaryInputStream => ValueType::Binary,
            ValueType::TableInputStream(o) => ValueType::Table(ColumnType::materialize(o)?),
            ValueType::TableOutputStream(_) => {
                return command_error("Can't materialize `$table_output_stream`");
            }
            ValueType::Table(r) => ValueType::Table(ColumnType::materialize(r)?),
            ValueType::List(l) => ValueType::List(Box::from(l.materialize()?)),
            ValueType::Dict(k, v) => {
                ValueType::Dict(Box::from(k.materialize()?), Box::from(v.materialize()?))
            }

            ValueType::OneOf(types) => ValueType::OneOf(
                types
                    .iter()
                    .map(|t| t.materialize())
                    .collect::<CrushResult<Vec<_>>>()?,
            ),
        })
    }

    pub fn is_hashable(&self) -> bool {
        match self {
            ValueType::Scope
            | ValueType::List(_)
            | ValueType::Dict(_, _)
            | ValueType::Command
            | ValueType::BinaryInputStream
            | ValueType::TableInputStream(_)
            | ValueType::Struct
            | ValueType::Table(_) => false,
            ValueType::OneOf(types) => types.iter().all(|t| t.is_hashable()),
            _ => true,
        }
    }

    pub fn is_comparable(&self) -> bool {
        self.is_hashable()
    }

    pub fn parse(&self, s: &str) -> CrushResult<Value> {
        match self {
            ValueType::String => Ok(Value::from(s)),
            ValueType::Integer => match s.parse::<i128>() {
                Ok(n) => Ok(Value::Integer(n)),
                Err(e) => error(e.to_string().as_str()),
            },
            ValueType::Glob => Ok(Value::Glob(Glob::new(s))),
            ValueType::Regex => Ok(Value::Regex(s.to_string(), Regex::new(s)?)),
            ValueType::File => Ok(Value::from(s)),
            ValueType::Float => Ok(Value::Float(s.parse::<f64>()?)),
            ValueType::Bool => Ok(Value::Bool(s.parse::<bool>()?)),
            _ => error(format!("Can't parse string into value of type `{}`", self)),
        }
    }

    pub fn is_parametrized(&self) -> bool {
        match self {
            ValueType::List(_)
            | ValueType::Dict(_, _)
            | ValueType::TableOutputStream(_)
            | ValueType::TableInputStream(_)
            | ValueType::Table(_)
            | ValueType::OneOf(_) => true,
            _ => false,
        }
    }

    pub fn subfmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.is_parametrized() {
            f.write_str("$(")?;
        } else {
            f.write_str("$")?;
        }
        self.fmt(f)?;
        if self.is_parametrized() {
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl Help for ValueType {
    fn signature(&self) -> String {
        format!("type {}", self)
    }

    fn short_help(&self) -> String {
        match self {
            ValueType::String => {
                "Textual data, stored as an immutable sequence of unicode code points."
            }
            ValueType::Integer => "A numeric type representing an integer number.",
            ValueType::Time => "A point in time with nanosecond precision.",
            ValueType::Duration => "A difference between two points in time.",
            ValueType::Glob => "A pattern containing wildcards.",
            ValueType::Regex => "An advanced pattern that can be used for matching and replacing.",
            ValueType::Command => "A piece fo code that can be called.",
            ValueType::File => "Any type of file.",
            ValueType::TableInputStream(_) => "An input stream of table rows.",
            ValueType::TableOutputStream(_) => "An output stream of table rows.",
            ValueType::Table(_) => "A table of rows.",
            ValueType::Struct => "A mapping from name to value.",
            ValueType::List(_) => "A mutable list of items, usually of the same type.",
            ValueType::Dict(_, _) => "A mutable mapping from one set of values to another.",
            ValueType::Scope => "A scope in the Crush namespace.",
            ValueType::Bool => "True or false.",
            ValueType::Float => {
                "A numeric type representing any number with floating point precision."
            }
            ValueType::Empty => "Nothing.",
            ValueType::Any => "Any type.",
            ValueType::BinaryInputStream => "A stream of binary data.",
            ValueType::Binary => "Binary data.",
            ValueType::Type => "A type.",
            ValueType::OneOf(types) => {
                return format!("One of {}", types.iter().map(|t| t.to_string()).join(", "));
            }
        }
        .to_string()
    }

    fn long_help(&self) -> Option<String> {
        let mut lines = match self {
            ValueType::Duration => {
                vec![
                    "A duration instance has nanosecond precision. It is represented internally".to_string(),
                    "as two 64 bit numbers, one for the number of seconds, and one for the".to_string(),
                    "nanosecond remainder".to_string(),
                    "".to_string(),
                    "durations are signed, i.e. they can be used to denote a negative span of time.".to_string(),
                    "".to_string(),
                ]
            }
            ValueType::Time => {
                vec![
                    "All time instances use the local time zone.".to_string(),
                    "".to_string(),
                    "A time instance has nanosecond precision. It is represented internally"
                        .to_string(),
                    "as two 64 bit numbers, one for the number of seconds since the Unix epoc,"
                        .to_string(),
                    "and one for the nanosecond remainder".to_string(),
                    "".to_string(),
                ]
            }
            ValueType::Integer => {
                vec![
                    "A Crush integer uses signed 128 bit precision. This means that the highest"
                        .to_string(),
                    format!("number that can be represented is {},", i128::MAX),
                    format!("and the lowest is {}.", i128::MIN),
                    "".to_string(),
                ]
            }
            ValueType::Float => {
                vec![
                    "A Crush float is a IEEE 754 64-bit (double precision) floating point number."
                        .to_string(),
                ]
            }
            ValueType::Bool => {
                vec!["A boolean value is one of `$true` or `$false`.".to_string()]
            }
            ValueType::Struct => {
                vec![
                    "To create a simple immutable struct, use the `struct:of` command. To create a mutable struct that supports inheritance, use the `class` command.".to_string(),
                ]
            }
            ValueType::Empty => {
                vec![
                    "The empty type is returned by commands that don't return any value."
                        .to_string(),
                ]
            }
            _ => Vec::new(),
        };

        let mut keys: Vec<_> = self.fields().into_iter().collect();
        keys.sort_by(|x, y| x.0.cmp(&y.0));

        long_help_methods(&keys, &mut lines);
        Some(lines.join("\n"))
    }
}

fn long_help_methods(fields: &Vec<(&String, &Command)>, lines: &mut Vec<String>) {
    for (k, v) in fields {
        lines.push(format!(" * `{}` {}", k, v.short_help()));
    }
}

impl Display for ValueType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueType::String => f.write_str("string"),
            ValueType::Integer => f.write_str("integer"),
            ValueType::Time => f.write_str("time"),
            ValueType::Duration => f.write_str("duration"),
            ValueType::Glob => f.write_str("glob"),
            ValueType::Regex => f.write_str("re"),
            ValueType::Command => f.write_str("command"),
            ValueType::File => f.write_str("file"),
            ValueType::TableInputStream(columns) => {
                f.write_str("table_input_stream")?;
                for i in columns.iter() {
                    f.write_str(" ")?;
                    i.fmt(f)?;
                }
                Ok(())
            }
            ValueType::TableOutputStream(columns) => {
                f.write_str("table_output_stream")?;
                for i in columns.iter() {
                    f.write_str(" ")?;
                    i.fmt(f)?;
                }
                Ok(())
            }
            ValueType::Table(columns) => {
                f.write_str("table")?;
                for i in columns.iter() {
                    f.write_str(" ")?;
                    i.fmt(f)?;
                }
                Ok(())
            }
            ValueType::Struct => f.write_str("struct"),
            ValueType::List(value_type) => {
                f.write_str("list ")?;
                value_type.subfmt(f)
            }
            ValueType::Dict(key_type, value_type) => {
                f.write_str("dict ")?;
                key_type.subfmt(f)?;
                f.write_str(" ")?;
                value_type.subfmt(f)
            }
            ValueType::Scope => f.write_str("scope"),
            ValueType::Bool => f.write_str("bool"),
            ValueType::Float => f.write_str("float"),
            ValueType::Empty => f.write_str("empty"),
            ValueType::Any => f.write_str("any"),
            ValueType::BinaryInputStream => f.write_str("binary_stream"),
            ValueType::Binary => f.write_str("binary"),
            ValueType::Type => f.write_str("type"),
            ValueType::OneOf(types) => {
                f.write_str("one_of")?;
                for i in types.iter() {
                    f.write_str(" ")?;
                    i.subfmt(f)?;
                }
                Ok(())
            }
        }
    }
}
