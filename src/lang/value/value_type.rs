/// All the different types a value can have.

use crate::lang::command::{Command, OutputType};
use crate::lang::errors::{error, CrushResult, argument_error_legacy};
use crate::lang::help::Help;
use crate::lang::{data::table::ColumnType, value::Value};
use crate::builtins::types;
use crate::util::glob::Glob;
use ordered_map::OrderedMap;
use regex::Regex;
use std::cmp::max;
use std::fmt::{Display, Formatter};
use std::sync::OnceLock;
use crate::lang::command::OutputType::Known;

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

    pub fn either(_options: Vec<ValueType>) -> ValueType {
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
            _ => empty_methods(),
        }
    }

    pub fn is(&self, value: &Value) -> bool {
        (*self == ValueType::Any) || (*self == value.value_type())
    }

    pub fn is_compatible_with(&self, pattern: &ValueType) -> bool {
        (*self == ValueType::Any) || (self == pattern)
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
            ValueType::TableOutputStream(_) => return argument_error_legacy("Can't materialize binary_output_stream"),
            ValueType::Table(r) => ValueType::Table(ColumnType::materialize(r)?),
            ValueType::List(l) => ValueType::List(Box::from(l.materialize()?)),
            ValueType::Dict(k, v) => {
                ValueType::Dict(Box::from(k.materialize()?), Box::from(v.materialize()?))
            }
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
            _ => error("Failed to parse cell"),
        }
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
            ValueType::Time => "A point in time with nanosecond precision",
            ValueType::Duration => "A difference between two points in time",
            ValueType::Glob => "A pattern containing wildcards",
            ValueType::Regex => "An advanced pattern that can be used for matching and replacing",
            ValueType::Command => "A piece fo code that can be called",
            ValueType::File => "Any type of file",
            ValueType::TableInputStream(_) => "An input stream of table rows",
            ValueType::TableOutputStream(_) => "An output stream of table rows",
            ValueType::Table(_) => "A table of rows",
            ValueType::Struct => "A mapping from name to value",
            ValueType::List(_) => "A mutable list of items, usually of the same type",
            ValueType::Dict(_, _) => "A mutable mapping from one set of values to another",
            ValueType::Scope => "A scope in the Crush namespace",
            ValueType::Bool => "True or false",
            ValueType::Float => {
                "A numeric type representing any number with floating point precision"
            }
            ValueType::Empty => "Nothing",
            ValueType::Any => "Any type",
            ValueType::BinaryInputStream => "A stream of binary data",
            ValueType::Binary => "Binary data",
            ValueType::Type => "A type",
        }
            .to_string()
    }

    fn long_help(&self) -> Option<String> {
        let mut lines = match self {
            ValueType::Time => {
                vec!["    All time instances use the local time zone.\n".to_string()]
            }
            _ => { Vec::new() }
        };

        let mut keys: Vec<_> = self.fields().into_iter().collect();
        keys.sort_by(|x, y| x.0.cmp(&y.0));

        long_help_methods(&keys, &mut lines);
        Some(lines.join("\n"))
    }
}

fn long_help_methods(fields: &Vec<(&String, &Command)>, lines: &mut Vec<String>) {
    let mut max_len = 0;
    for (k, _) in fields {
        max_len = max(max_len, k.len());
    }
    for (k, v) in fields {
        lines.push(format!(
            "    * {}  {}{}",
            k,
            " ".repeat(max_len - k.len()),
            v.help().short_help()
        ));
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
            ValueType::Regex => f.write_str("regex"),
            ValueType::Command => f.write_str("command"),
            ValueType::File => f.write_str("file"),
            ValueType::TableInputStream(o) => {
                f.write_str("table_input_stream")?;
                for i in o.iter() {
                    f.write_str(" ")?;
                    i.fmt(f)?;
                }
                Ok(())
            }
            ValueType::TableOutputStream(o) => {
                f.write_str("table_output_stream")?;
                for i in o.iter() {
                    f.write_str(" ")?;
                    i.fmt(f)?;
                }
                Ok(())
            }
            ValueType::Table(o) => {
                f.write_str("table")?;
                for i in o.iter() {
                    f.write_str(" ")?;
                    i.fmt(f)?;
                }
                Ok(())
            }
            ValueType::Struct => f.write_str("struct"),
            ValueType::List(l) => {
                f.write_str("list ")?;
                l.fmt(f)
            }
            ValueType::Dict(k, v) => {
                f.write_str("dict ")?;
                k.fmt(f)?;
                f.write_str(" ")?;
                v.fmt(f)
            }
            ValueType::Scope => f.write_str("scope"),
            ValueType::Bool => f.write_str("bool"),
            ValueType::Float => f.write_str("float"),
            ValueType::Empty => f.write_str("empty"),
            ValueType::Any => f.write_str("any"),
            ValueType::BinaryInputStream => f.write_str("binary_stream"),
            ValueType::Binary => f.write_str("binary"),
            ValueType::Type => f.write_str("type"),
        }
    }
}
