use crate::lang::value::{Value, ValueType};
use std::collections::{HashSet, HashMap};
use std::path::Path;
use std::fs::File;
use crate::lang::errors::{CrushResult, to_crush_error, error, CrushError};
use std::io::{Write, Seek, Read, Cursor};
use prost::Message;
use std::convert::TryFrom;
use crate::util::identity_arc::Identity;
use crate::lang::serialization;
use crate::lang::serialization::SerializedValue;
use crate::lang::serialization::SerializationNode;
use crate::lang::serialization::serialization_node;
use crate::lang::serialization::r#type::SimpleTypeKind;
use crate::lang::serialization::r#type::Type::SimpleType;
use chrono::Duration;
use crate::lang::list::List;
use std::ops::Deref;

pub struct SerializationState {
    pub with_id: HashMap<u64, usize>,
    pub hashable: HashMap<Value, usize>,
}

pub struct DeserializationState {
    pub deserialized: HashMap<usize, Value>,
}

pub fn serialize(value: &Value, destination: &Path) -> CrushResult<()> {
    let mut res = SerializedValue::default();
    let mut state = SerializationState {
        with_id: HashMap::new(),
        hashable: HashMap::new(),
    };
    res.root = value.clone().materialize().serialize(&mut res.nodes, &mut state)? as u64;

    let mut buf = Vec::new();
    buf.reserve(res.encoded_len());
    res.encode(&mut buf).unwrap();

    let mut file_buffer = to_crush_error(File::create(destination))?;
    let mut pos = 0;

    while pos < buf.len() {
        let bytes_written = to_crush_error(file_buffer.write(&buf[pos..]))?;
        pos += bytes_written;
    }
    Ok(())
}

pub fn deserialize(source: &Path) -> CrushResult<Value> {
    let mut buf = Vec::new();
    let mut file_buffer = to_crush_error(File::open(source))?;
    buf.reserve(to_crush_error(source.metadata())?.len() as usize);
    file_buffer.read_to_end(&mut buf);

    let mut state = DeserializationState {
        deserialized: HashMap::new(),
    };

    let mut res = SerializedValue::decode(&mut Cursor::new(buf)).unwrap();

    println!("AAA {:?}", res);

    Ok(*Value::deserialize(res.root as usize, &res.nodes, &mut state)?)
}

pub trait Serializable {
    fn deserialize(id: usize, nodes: &Vec<SerializationNode>, state: &mut DeserializationState) -> CrushResult<Box<Self>>;
    fn serialize(&self, nodes: &mut Vec<SerializationNode>, state: &mut SerializationState) -> CrushResult<usize>;
}

impl Serializable for ValueType {
    fn deserialize(id: usize, nodes: &Vec<SerializationNode>, state: &mut DeserializationState) -> CrushResult<Box<ValueType>> {
        if let serialization_node::Value::Type(outer_type) = nodes[id].value.as_ref().unwrap() {
            match outer_type.r#type {
                Some(SimpleType(simple_type)) => {
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
                    Ok(Box::from(vt))
                }
                _ => unimplemented!(),
            }
        } else {
            error("Invalid type")
        }
    }

    fn serialize(&self, nodes: &mut Vec<SerializationNode>, state: &mut SerializationState) -> CrushResult<usize> {
        let tt = match self {
            ValueType::String => SimpleTypeKind::String,
            ValueType::Integer => SimpleTypeKind::Integer,
            ValueType::Time => SimpleTypeKind::Time,
            ValueType::Duration => SimpleTypeKind::Duration,
            ValueType::Field => SimpleTypeKind::Field,
            ValueType::Glob => SimpleTypeKind::Glob,
            ValueType::Regex => SimpleTypeKind::Regex,
            ValueType::Command => SimpleTypeKind::Command,
            ValueType::File => SimpleTypeKind::File,
            ValueType::Struct => SimpleTypeKind::Struct,
            ValueType::Scope => SimpleTypeKind::Scope,
            ValueType::Bool => SimpleTypeKind::Bool,
            ValueType::Float => SimpleTypeKind::Float,
            ValueType::Empty => SimpleTypeKind::Empty,
            ValueType::Any => SimpleTypeKind::Any,
            ValueType::Binary => SimpleTypeKind::Binary,
            ValueType::Type => SimpleTypeKind::Type,
            ValueType::List(_) => unimplemented!(),
            ValueType::Dict(_, _) => unimplemented!(),
            ValueType::Table(_) => unimplemented!(),
            ValueType::TableStream(_) => return error("Can't serialize streams"),
            ValueType::BinaryStream => return error("Can't serialize streams"),
        };

        let mut node = serialization::SerializationNode::default();
        let mut ttt = serialization::Type::default();
        ttt.r#type = Some(SimpleType(tt as i32));
        node.value = Some(serialization_node::Value::Type(ttt));
        let idx = nodes.len();
        nodes.push(node);
        Ok(idx)
    }
}


impl Serializable for Value {
    fn deserialize(id: usize, nodes: &Vec<SerializationNode>, state: &mut DeserializationState) -> CrushResult<Box<Value>> {
        match nodes[id].value.as_ref().unwrap() {
            serialization_node::Value::String(s) => {
                Ok(Box::from(Value::string(s.as_str())))
            }
            serialization_node::Value::SmallInteger(i) => {
                Ok(Box::from(Value::Integer(*i as i128)))
            }
            serialization_node::Value::Duration(d) => {
                let dd = Duration::seconds(d.secs) + Duration::nanoseconds(d.nanos as i64);
                Ok(Box::from(Value::Duration(dd)))
            }
            serialization_node::Value::List(l) => {
                if state.deserialized.contains_key(&id) {
                    Ok(Box::from(state.deserialized[&id].clone()))
                } else {
                    let element_type_value = Value::deserialize(l.element_type as usize, nodes, state)?;
                    if let Value::Type(element_type) = *element_type_value {
                        let list = List::new(element_type, vec![]);
                        let res = Value::List(list.clone());
                        state.deserialized.insert(id, res.clone());

                        for el_id in &l.elements {
                            list.append(&mut vec![*Value::deserialize(*el_id as usize, nodes, state)?])?;
                        }

                        Ok(Box::from(res))
                    } else {
                        error("Deserialization error")
                    }
                }
            }

            serialization_node::Value::Type(_) => {
                Ok(Box::from(Value::Type(*ValueType::deserialize(id, nodes, state)?)))
            }
            _ => unimplemented!(),
        }
    }

    fn serialize(&self, nodes: &mut Vec<SerializationNode>, state: &mut SerializationState) -> CrushResult<usize> {
        if self.value_type().is_hashable() {
            if state.hashable.contains_key(self) {
                return Ok(state.hashable[self]);
            }
        }

        match self {
            Value::String(s) => {
                let mut node = SerializationNode::default();
                node.value = Some(serialization_node::Value::String(s.to_string()));
                let idx = nodes.len();
                state.hashable.insert(self.clone(), idx);
                nodes.push(node);
                Ok(idx)
            }
            Value::Integer(s) => {
                let mut node = SerializationNode::default();
                match i64::try_from(*s) {
                    Ok(v) => {
                        node.value = Some(serialization_node::Value::SmallInteger(v));
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
                let mut node = SerializationNode::default();
                let mut dd = serialization::Duration::default();
                dd.secs = d.num_seconds();
                dd.nanos = 0;
                node.value = Some(serialization_node::Value::Duration(dd));
                let idx = nodes.len();
                state.hashable.insert(self.clone(), idx);
                nodes.push(node);
                Ok(idx)
            }
            Value::Type(t) => {
                t.serialize(nodes, state)
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
                    node.value = Some(serialization_node::Value::List(ll));
                    nodes[idx] = node;
                }
                Ok(state.with_id[&id])
            }

            Value::Table(t) => {
                let idx = nodes.len();
                nodes.push(serialization::SerializationNode::default());
                let mut stable = serialization::Table::default();
                nodes[idx].value = Some(serialization_node::Value::Table(stable));
                Ok(idx)
            }

            _ => unimplemented!(),
        }
    }
}
