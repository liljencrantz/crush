use crate::lang::command::Command;
use crate::lang::errors::{error, mandate, to_crush_error, CrushResult};
use crate::lang::help::Help;
use crate::lang::parser::parse_name;
use crate::lang::{table::ColumnType, value::Value};
use crate::lib::types;
use crate::util::glob::Glob;
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use regex::Regex;
use std::cmp::max;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum ValueType {
    String,
    Integer,
    Time,
    Duration,
    Field,
    Glob,
    Regex,
    Command,
    File,
    TableStream(Vec<ColumnType>),
    Table(Vec<ColumnType>),
    Struct,
    List(Box<ValueType>),
    Dict(Box<ValueType>, Box<ValueType>),
    Scope,
    Bool,
    Float,
    Empty,
    Any,
    BinaryStream,
    Binary,
    Type,
}

lazy_static! {
    pub static ref EMPTY_METHODS: OrderedMap<String, Command> = OrderedMap::new();
}

impl ValueType {
    pub fn fields(&self) -> &OrderedMap<String, Command> {
        match self {
            ValueType::List(_) => &types::list::METHODS,
            ValueType::Dict(_, _) => &types::dict::METHODS,
            ValueType::String => &types::string::METHODS,
            ValueType::File => &types::file::METHODS,
            ValueType::Regex => &types::re::METHODS,
            ValueType::Glob => &types::glob::METHODS,
            ValueType::Integer => &types::integer::METHODS,
            ValueType::Float => &types::float::METHODS,
            ValueType::Duration => &types::duration::METHODS,
            ValueType::Time => &types::time::METHODS,
            ValueType::Table(_) => &types::table::METHODS,
            ValueType::TableStream(_) => &types::table_stream::METHODS,
            ValueType::Binary => &types::binary::METHODS,
            ValueType::Scope => &types::scope::METHODS,
            _ => &EMPTY_METHODS,
        }
    }

    pub fn is(&self, value: &Value) -> bool {
        (*self == ValueType::Any) || (*self == value.value_type())
    }

    pub fn materialize(&self) -> ValueType {
        match self {
            ValueType::String
            | ValueType::Integer
            | ValueType::Time
            | ValueType::Duration
            | ValueType::Field
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
            ValueType::BinaryStream => ValueType::Binary,
            ValueType::TableStream(o) => ValueType::Table(ColumnType::materialize(o)),
            ValueType::Table(r) => ValueType::Table(ColumnType::materialize(r)),
            ValueType::List(l) => ValueType::List(Box::from(l.materialize())),
            ValueType::Dict(k, v) => {
                ValueType::Dict(Box::from(k.materialize()), Box::from(v.materialize()))
            }
        }
    }

    pub fn is_hashable(&self) -> bool {
        match self {
            ValueType::Scope
            | ValueType::List(_)
            | ValueType::Dict(_, _)
            | ValueType::Command
            | ValueType::BinaryStream
            | ValueType::TableStream(_)
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
            ValueType::String => Ok(Value::string(s)),
            ValueType::Integer => match s.parse::<i128>() {
                Ok(n) => Ok(Value::Integer(n)),
                Err(e) => error(e.to_string().as_str()),
            },
            ValueType::Field => Ok(Value::Field(mandate(parse_name(s), "Invalid field name")?)),
            ValueType::Glob => Ok(Value::Glob(Glob::new(s))),
            ValueType::Regex => Ok(Value::Regex(s.to_string(), to_crush_error(Regex::new(s))?)),
            ValueType::File => Ok(Value::string(s)),
            ValueType::Float => Ok(Value::Float(to_crush_error(s.parse::<f64>())?)),
            ValueType::Bool => Ok(Value::Bool(to_crush_error(s.parse::<bool>())?)),
            _ => error("Failed to parse cell"),
        }
    }
}

impl Help for ValueType {
    fn signature(&self) -> String {
        format!("type {}", self.to_string())
    }

    fn short_help(&self) -> String {
        match self {
            ValueType::String => {
                "Textual data, stored as an immutable sequence of unicode code points."
            }
            ValueType::Integer => "A numeric type representing an integer number.",
            ValueType::Time => "A point in time with nanosecond precision",
            ValueType::Duration => "A difference between two points in time",
            ValueType::Field => "A field is used to represent a path into a datastructure",
            ValueType::Glob => "A pattern containing wildcards",
            ValueType::Regex => "An advanced pattern that can be used for matching and replacing",
            ValueType::Command => "A piece fo code that can be called",
            ValueType::File => "Any type of file",
            ValueType::TableStream(_) => "A stream of table rows",
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
            ValueType::BinaryStream => "A stream of binary data",
            ValueType::Binary => "Binary data",
            ValueType::Type => "A type",
        }
        .to_string()
    }

    fn long_help(&self) -> Option<String> {
        let mut lines = Vec::new();

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

impl ToString for ValueType {
    fn to_string(&self) -> String {
        match self {
            ValueType::String => "string".to_string(),
            ValueType::Integer => "integer".to_string(),
            ValueType::Time => "time".to_string(),
            ValueType::Duration => "duration".to_string(),
            ValueType::Field => "field".to_string(),
            ValueType::Glob => "glob".to_string(),
            ValueType::Regex => "regex".to_string(),
            ValueType::Command => "command".to_string(),
            ValueType::File => "file".to_string(),
            ValueType::TableStream(o) => format!(
                "table_stream {}",
                o.iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
            ValueType::Table(r) => format!(
                "table {}",
                r.iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
            ValueType::Struct => "struct".to_string(),
            ValueType::List(l) => format!("list {}", l.to_string()),
            ValueType::Dict(k, v) => format!("dict {} {}", k.to_string(), v.to_string()),
            ValueType::Scope => "scope".to_string(),
            ValueType::Bool => "bool".to_string(),
            ValueType::Float => "float".to_string(),
            ValueType::Empty => "empty".to_string(),
            ValueType::Any => "any".to_string(),
            ValueType::BinaryStream => "binary_stream".to_string(),
            ValueType::Binary => "binary".to_string(),
            ValueType::Type => "type".to_string(),
        }
    }
}
