mod value_definition;
mod value_type;

use std::cmp::Ordering;
use std::hash::Hasher;
use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, Local};
use regex::Regex;

use crate::{
    util::file::cwd,
    lang::table::Table,
    lang::errors::{error, to_crush_error},
    util::glob::Glob,
};
use crate::lang::{list::List, dict::Dict, table::ColumnType, binary::BinaryReader, table::TableReader, list::ListReader, dict::DictReader};
use crate::lang::errors::{CrushResult, argument_error};
use chrono::Duration;
use crate::util::time::duration_format;
use crate::lang::scope::Scope;
use crate::lang::r#struct::Struct;
use crate::lang::stream::{streams, Readable, InputStream};

pub use value_type::ValueType;
pub use value_definition::ValueDefinition;
use crate::lang::command::CrushCommand;
use std::collections::HashMap;
use crate::lang::pretty_printer::format_buffer;
use crate::util::regex::RegexFileMatcher;
use crate::lang::serialize::{SerializationState, DeserializationState, Serializable};
use crate::lang::serialization;
use std::sync::atomic::AtomicU64;
use std::mem::transmute;
use crate::util::identity_arc::Identity;
use std::convert::{Infallible, TryFrom};


pub enum Value {
    String(Box<str>),
    Integer(i128),
    Time(DateTime<Local>),
    Duration(Duration),
    Field(Vec<Box<str>>),
    Glob(Glob),
    Regex(Box<str>, Regex),
    Command(Box<dyn CrushCommand + Send + Sync>),
    TableStream(InputStream),
    File(Box<Path>),
    Table(Table),
    Struct(Struct),
    List(List),
    Dict(Dict),
    Scope(Scope),
    Bool(bool),
    Float(f64),
    Empty(),
    BinaryStream(Box<dyn BinaryReader + Send + Sync>),
    Binary(Vec<u8>),
    Type(ValueType),
}

impl ToString for Value {
    fn to_string(&self) -> String {
        return match self {
            Value::String(val) => val.to_string(),
            Value::Integer(val) => val.to_string(),
            Value::Time(val) => val.format("%Y-%m-%d %H:%M:%S %z").to_string(),
            Value::Field(val) => format!(r"^{}", val.join(":")),
            Value::Glob(val) => val.to_string(),
            Value::Regex(val, _) => format!(r#"re"{}""#, val),
            Value::File(val) => val.to_str().unwrap_or("<invalid filename>").to_string(),
            Value::List(l) => l.to_string(),
            Value::Duration(d) => duration_format(d),
            Value::Scope(env) => env.to_string(),
            Value::Bool(v) => (if *v { "true" } else { "false" }).to_string(),
            Value::Dict(d) => d.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Binary(v) => format_buffer(v, true),
            Value::Type(t) => t.to_string(),
            Value::Struct(s) => s.to_string(),
            _ => format!("<{}>", self.value_type().to_string()),
        };
    }
}

fn add_keys<T>(map: &HashMap<Box<str>, T>, res: &mut Vec<Box<str>>) {
    res.append(&mut map.keys().map(|k| k.to_string().into_boxed_str()).collect());
}

impl Value {
    pub fn field(&self, name: &str) -> Option<Value> {
        match self {
            Value::Struct(s) => s.get(name),
            Value::Scope(subenv) =>
                subenv
                    .get(name)
                    .or_else(|| {
                        self.value_type()
                            .fields()
                            .get(&Box::from(name))
                            .map(|m| Value::Command(m.as_ref().clone()))
                    }),
            Value::Type(t) =>
                t.fields()
                    .get(&Box::from(name))
                    .map(|m| Value::Command(m.as_ref().clone())),
            _ =>
                self.value_type()
                    .fields()
                    .get(&Box::from(name))
                    .map(|m| Value::Command(m.as_ref().clone())),
        }
    }

    pub fn fields(&self) -> Vec<Box<str>> {
        let mut res = Vec::new();
        match self {
            Value::Struct(s) => {
                res.append(&mut s.keys())
            }
//            Value::Scope(subenv) => subenv.get(name),
            Value::Type(t) => {
                add_keys(t.fields(), &mut res)
            }
            _ => add_keys(self.value_type().fields(), &mut res),
        }
        res.sort_by(|x, y| x.cmp(y));

        res
    }

    pub fn path(&self, name: &str) -> Option<Value> {
        match self {
            Value::File(s) => Some(Value::File(s.join(name).into_boxed_path())),
            _ => return None,
        }
    }

    pub fn alignment(&self) -> Alignment {
        return match self {
            Value::Time(_) | Value::Duration(_) | Value::Integer(_) => Alignment::Right,
            _ => Alignment::Left,
        };
    }

    pub fn empty_table_stream() -> Value {
        let (_s, r) = streams(vec![]);
        Value::TableStream(r)
    }

    pub fn string(s: &str) -> Value {
        Value::String(Box::from(s))
    }

    pub fn readable(&self) -> Option<Box<dyn Readable>> {
        match self {
            Value::TableStream(s) => Some(Box::from(s.clone())),
            Value::Table(r) => Some(Box::from(TableReader::new(r.clone()))),
            Value::List(l) => Some(Box::from(ListReader::new(l.clone(), "value"))),
            Value::Dict(d) => Some(Box::from(DictReader::new(d.clone()))),
            _ => None,
        }
    }

    pub fn value_type(&self) -> ValueType {
        match self {
            Value::String(_) => ValueType::String,
            Value::Integer(_) => ValueType::Integer,
            Value::Time(_) => ValueType::Time,
            Value::Field(_) => ValueType::Field,
            Value::Glob(_) => ValueType::Glob,
            Value::Regex(_, _) => ValueType::Regex,
            Value::Command(_) => ValueType::Command,
            Value::File(_) => ValueType::File,
            Value::TableStream(o) => ValueType::TableStream(o.types().clone()),
            Value::Table(r) => ValueType::Table(r.types().clone()),
            Value::Struct(_) => ValueType::Struct,
            Value::List(l) => l.list_type(),
            Value::Duration(_) => ValueType::Duration,
            Value::Scope(_) => ValueType::Scope,
            Value::Bool(_) => ValueType::Bool,
            Value::Dict(d) => d.dict_type(),
            Value::Float(_) => ValueType::Float,
            Value::Empty() => ValueType::Empty,
            Value::BinaryStream(_) => ValueType::BinaryStream,
            Value::Binary(_) => ValueType::Binary,
            Value::Type(_) => ValueType::Type,
        }
    }

    pub fn file_expand(&self, v: &mut Vec<Box<Path>>) -> CrushResult<()> {
        match self {
            Value::String(s) => v.push(Box::from(Path::new(s.as_ref()))),
            Value::File(p) => v.push(p.clone()),
            Value::Glob(pattern) => pattern.glob_files(&cwd()?, v)?,
            Value::Regex(_, re) => re.match_files(&cwd()?, v),
            Value::TableStream(s) => {
                let t = s.types();
                if t.len() == 1 && t[0].cell_type == ValueType::File {
                    loop {
                        match s.recv() {
                            Ok(row) => {
                                if let Value::File(f) = row.into_vec().remove(0) {
                                    v.push(f);
                                }
                            }
                            Err(_) => break,
                        }
                    }
                } else {
                    return argument_error("Table stream must contain one column of type file");
                }
            }
            _ => return error("Expected a file name"),
        }
        Ok(())
    }

    pub fn materialize(self) -> Value {
        match self {
            Value::TableStream(output) => {
                let mut rows = Vec::new();
                loop {
                    match output.recv() {
                        Ok(r) => rows.push(r.materialize()),
                        Err(_) => break,
                    }
                }
                Value::Table(Table::new(ColumnType::materialize(output.types()), rows))
            }
            Value::BinaryStream(mut s) => {
                let mut vec = Vec::new();
                std::io::copy(s.as_mut(), &mut vec).unwrap();
                Value::Binary(vec)
            }
            Value::Table(r) => Value::Table(r.materialize()),
            Value::Dict(d) => Value::Dict(d.materialize()),
            Value::Struct(r) => Value::Struct(r.materialize()),
            Value::List(l) => Value::List(l.materialize()),
            _ => self,
        }
    }

    pub fn cast(self, new_type: ValueType) -> CrushResult<Value> {
        if self.value_type() == new_type {
            return Ok(self);
        }

        match (&self, &new_type) {
            (Value::Integer(i), ValueType::Bool) =>
                return Ok(Value::Bool(if *i == 0 { false } else { true })),
            _ => {}
        }

        let str_val = self.to_string();

        match new_type {
            ValueType::File => Ok(Value::File(Box::from(Path::new(str_val.as_str())))),
            ValueType::Glob => Ok(Value::Glob(Glob::new(str_val.as_str()))),
            ValueType::Integer => to_crush_error(str_val.parse::<i128>()).map(|v| Value::Integer(v)),
            ValueType::Field => Ok(Value::Field(vec![str_val.into_boxed_str()])),
            ValueType::Regex => to_crush_error(Regex::new(str_val.as_str()).map(|v| Value::Regex(str_val.into_boxed_str(), v))),
            ValueType::Binary => Ok(Value::Binary(str_val.bytes().collect())),
            ValueType::Float => Ok(Value::Float(to_crush_error(f64::from_str(&str_val))?)),
            ValueType::Bool => Ok(Value::Bool(match str_val.as_str() {
                "true" => true,
                "false" => false,
                _ => return error("Can't cast to boolean")
            })),
            ValueType::String => Ok(Value::String(str_val.into_boxed_str())),
            ValueType::Time => error("invalid cast"),
            ValueType::Duration => Ok(Value::Duration(Duration::seconds(to_crush_error(i64::from_str(&str_val))?))),
            ValueType::Command => error("invalid cast"),
            ValueType::TableStream(_) => error("invalid cast"),
            ValueType::Table(_) => error("invalid cast"),
            ValueType::Struct => error("invalid cast"),
            ValueType::List(_) => error("invalid cast"),
            ValueType::Dict(_, _) => error("invalid cast"),
            ValueType::Scope => error("Invalid cast"),
            ValueType::Empty => error("Invalid cast"),
            ValueType::Any => error("Invalid cast"),
            ValueType::BinaryStream => error("invalid cast"),
            ValueType::Type => error("invalid cast"),
        }
    }
}

impl Clone for Value {
    fn clone(&self) -> Self {
        match self {
            Value::String(v) => Value::String(v.clone()),
            Value::Integer(v) => Value::Integer(v.clone()),
            Value::Time(v) => Value::Time(v.clone()),
            Value::Field(v) => Value::Field(v.clone()),
            Value::Glob(v) => Value::Glob(v.clone()),
            Value::Regex(v, r) => Value::Regex(v.clone(), r.clone()),
            Value::Command(v) => Value::Command(v.as_ref().clone()),
            Value::File(v) => Value::File(v.clone()),
            Value::Table(r) => Value::Table(r.clone()),
            Value::Struct(r) => Value::Struct(r.clone()),
            Value::TableStream(s) => Value::TableStream(s.clone()),
            Value::List(l) => Value::List(l.clone()),
            Value::Duration(d) => Value::Duration(d.clone()),
            Value::Scope(e) => Value::Scope(e.clone()),
            Value::Bool(v) => Value::Bool(v.clone()),
            Value::Dict(d) => Value::Dict(d.clone()),
            Value::Float(f) => Value::Float(f.clone()),
            Value::Empty() => Value::Empty(),
            Value::BinaryStream(v) => Value::BinaryStream(v.as_ref().clone()),
            Value::Binary(v) => Value::Binary(v.clone()),
            Value::Type(t) => Value::Type(t.clone()),
        }
    }
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
            Value::Field(v) => v.hash(state),
            Value::Glob(v) => v.hash(state),
            Value::Regex(v, _) => v.hash(state),
            Value::Command(_) => {}
            Value::File(v) => v.hash(state),
            Value::Duration(d) => d.hash(state),
            Value::Bool(v) => v.hash(state),
            Value::Binary(v) => v.hash(state),
            Value::Struct(v) => v.hash(state),
            Value::Scope(_) | Value::Dict(_) | Value::Table(_) |
            Value::List(_) | Value::TableStream(_) | Value::Float(_)
            | Value::BinaryStream(_) => panic!("Can't hash output"),
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
        return match (self, other) {
            (Value::String(val1), Value::String(val2)) => val1 == val2,
            (Value::Integer(val1), Value::Integer(val2)) => val1 == val2,
            (Value::Time(val1), Value::Time(val2)) => val1 == val2,
            (Value::Duration(val1), Value::Duration(val2)) => val1 == val2,
            (Value::Field(val1), Value::Field(val2)) => val1 == val2,
            (Value::Glob(val1), Value::Glob(val2)) => val1 == val2,
            (Value::Regex(val1, _), Value::Regex(val2, _)) => val1 == val2,
            (Value::File(val1), Value::String(val2)) => file_result_compare(&Path::new(&val2.to_string()), val1.as_ref()),
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
        };
    }
}

impl Serializable for Value {
    fn deserialize(id: usize, nodes: &Vec<serialization::SerializationNode>, state: &mut DeserializationState) -> CrushResult<Value> {
        match nodes[id].value.as_ref().unwrap() {
            serialization::serialization_node::Value::String(s) => {
                Ok(Value::string(s.as_str()))
            }
            serialization::serialization_node::Value::SmallInteger(i) => {
                Ok(Value::Integer(*i as i128))
            }
            serialization::serialization_node::Value::Duration(d) => {
                let dd = Duration::seconds(d.secs) + Duration::nanoseconds(d.nanos as i64);
                Ok(Value::Duration(dd))
            }
            serialization::serialization_node::Value::List(l) => {
                if state.deserialized.contains_key(&id) {
                    Ok(state.deserialized[&id].clone())
                } else {
                    let element_type_value = Value::deserialize(l.element_type as usize, nodes, state)?;
                    if let Value::Type(element_type) = element_type_value {
                        let list = List::new(element_type, vec![]);
                        let res = Value::List(list.clone());
                        state.deserialized.insert(id, res.clone());

                        for el_id in &l.elements {
                            list.append(&mut vec![Value::deserialize(*el_id as usize, nodes, state)?])?;
                        }

                        Ok(res)
                    } else {
                        error("Deserialization error")
                    }
                }
            }

            serialization::serialization_node::Value::Type(outer_type) => {
                match outer_type.r#type {
                    Some(serialization::r#type::Type::SimpleType(simple_type)) => {
                        let y: serialization::r#type::SimpleType = unsafe { transmute(simple_type as i32) };
                        let vt = match simple_type {
                            0 => ValueType::String,
                            1 => ValueType::Integer,
                            2 => ValueType::File,
                            3 => ValueType::Float,
                            4 => ValueType::Command,
                            5 => ValueType::Binary,
                            6 => ValueType::Duration,
                            7 => ValueType::Field,
                            8 => ValueType::Glob,
                            9 => ValueType::Regex,
                            10 => ValueType::Scope,
                            11 => ValueType::Bool,
                            12 => ValueType::Empty,
                            13 => ValueType::Type,
                            14 => ValueType::Time,
                            15 => ValueType::Struct,
                            16 => ValueType::Any,
                            _ => return error("Unrecognised type")
                        };
                        Ok(Value::Type(vt))
                    }
                    _ => unimplemented!(),
                }
            }
            _ => unimplemented!(),
        }
    }

    fn serialize(&self, nodes: &mut Vec<serialization::SerializationNode>, state: &mut SerializationState) -> CrushResult<usize> {
        if self.value_type().is_hashable() {
            if state.hashable.contains_key(self) {
                return Ok(state.hashable[self]);
            }
        }

        match self {
            Value::String(s) => {
                let mut node = serialization::SerializationNode::default();
                node.value = Some(serialization::serialization_node::Value::String(s.to_string()));
                let idx = nodes.len();
                state.hashable.insert(self.clone(), idx);
                nodes.push(node);
                Ok(idx)
            }
            Value::Integer(s) => {
                let mut node = serialization::SerializationNode::default();
                match i64::try_from(*s) {
                    Ok(v) => {
                        node.value = Some(serialization::serialization_node::Value::SmallInteger(v));
                        let idx = nodes.len();
                        state.hashable.insert(self.clone(), idx);
                        nodes.push(node);
                        Ok(idx)
                    }
                    Err(_) => {
                        unimplemented!();
                    }
                }
            }
            Value::Duration(d) => {
                let mut node = serialization::SerializationNode::default();
                let mut dd = serialization::Duration::default();
                dd.secs = d.num_seconds();
                dd.nanos = 0;
                node.value = Some(serialization::serialization_node::Value::Duration(dd));
                let idx = nodes.len();
                state.hashable.insert(self.clone(), idx);
                nodes.push(node);
                Ok(idx)
            }
            Value::Type(t) => {
                let tt = match t {
                    ValueType::String => serialization::r#type::SimpleType::String,
                    ValueType::Integer => serialization::r#type::SimpleType::Integer,
                    ValueType::Time => serialization::r#type::SimpleType::Time,
                    ValueType::Duration => serialization::r#type::SimpleType::Duration,
                    ValueType::Field => serialization::r#type::SimpleType::Field,
                    ValueType::Glob => serialization::r#type::SimpleType::Glob,
                    ValueType::Regex => serialization::r#type::SimpleType::Regex,
                    ValueType::Command => serialization::r#type::SimpleType::Command,
                    ValueType::File => serialization::r#type::SimpleType::File,
                    ValueType::Struct => serialization::r#type::SimpleType::Struct,
                    ValueType::Scope => serialization::r#type::SimpleType::Scope,
                    ValueType::Bool => serialization::r#type::SimpleType::Bool,
                    ValueType::Float => serialization::r#type::SimpleType::Float,
                    ValueType::Empty => serialization::r#type::SimpleType::Empty,
                    ValueType::Any => serialization::r#type::SimpleType::Any,
                    ValueType::Binary => serialization::r#type::SimpleType::Binary,
                    ValueType::Type => serialization::r#type::SimpleType::Type,
                    ValueType::List(_) => unimplemented!(),
                    ValueType::Dict(_, _) => unimplemented!(),
                    ValueType::Table(_) => unimplemented!(),
                    ValueType::TableStream(_) => return error("Can't serialize streams"),
                    ValueType::BinaryStream => return error("Can't serialize streams"),
                };

                let mut node = serialization::SerializationNode::default();
                let mut ttt = serialization::Type::default();
                ttt.r#type = Some(serialization::r#type::Type::SimpleType(tt as i32));
                node.value = Some(serialization::serialization_node::Value::Type(ttt));
                let idx = nodes.len();
                nodes.push(node);
                Ok(idx)
            }
            Value::List(l) => {
                let id = l.id();
                if !state.with_id.contains_key(&id) {
                    let idx = nodes.len();
                    nodes.push(serialization::SerializationNode::default());
                    state.with_id.insert(id, idx);

                    let type_idx = Value::Type(l.element_type()).serialize(nodes, state)?;

                    let mut res = Vec::new();
                    let data = l.dump();
                    res.reserve(data.len());
                    let mut ll = serialization::List::default();
                    ll.elements = res;
                    for el in data {
                        ll.elements.push(el.serialize(nodes, state)? as u64)
                    }
                    ll.element_type = type_idx as u64;
                    let mut node = serialization::SerializationNode::default();
                    node.value = Some(serialization::serialization_node::Value::List(ll));
                    nodes[idx] = node;
                }
                Ok(state.with_id[&id])
            }
            _ => unimplemented!(),
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
            (Value::Field(val1), Value::Field(val2)) => Some(val1.cmp(val2)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_casts() {
        assert_eq!(Value::string("112432").cast(ValueType::Integer).is_err(), false);
        assert_eq!(Value::string("1d").cast(ValueType::Integer).is_err(), true);
        assert_eq!(Value::string("1d").cast(ValueType::Glob).is_err(), false);
        assert_eq!(Value::string("1d").cast(ValueType::File).is_err(), false);
        assert_eq!(Value::string("1d").cast(ValueType::Time).is_err(), true);
        assert_eq!(Value::string("fad").cast(ValueType::Field).is_err(), false);
    }

    #[test]
    fn test_duration_format() {
        assert_eq!(duration_format(&Duration::microseconds(0)), "0".to_string());
        assert_eq!(duration_format(&Duration::microseconds(1)), "0.000001".to_string());
        assert_eq!(duration_format(&Duration::microseconds(100)), "0.0001".to_string());
        assert_eq!(duration_format(&Duration::milliseconds(1)), "0.001".to_string());
        assert_eq!(duration_format(&Duration::milliseconds(1000)), "1".to_string());
        assert_eq!(duration_format(&Duration::milliseconds(1000 * 61)), "1:01".to_string());
        assert_eq!(duration_format(&Duration::milliseconds(1000 * 3601)), "1:00:01".to_string());
        assert_eq!(duration_format(&Duration::milliseconds(1000 * (3600 * 24 * 3 + 1))), "3d0:00:01".to_string());
        assert_eq!(duration_format(&Duration::milliseconds(1000 * (3600 * 24 * 365 * 10 + 1))), "10y0d0:00:01".to_string());
        assert_eq!(duration_format(&Duration::milliseconds(1000 * (3600 * 24 * 365 * 10 + 1) + 1)), "10y0d0:00:01".to_string());
    }
}
