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
use crate::lang::serialization::{SerializedValue};
use crate::lang::serialization::Element;
use crate::lang::serialization::element;
use crate::lang::serialization::r#type::SimpleTypeKind;
use crate::lang::serialization::r#type::Type::SimpleType;
use chrono::Duration;
use crate::lang::list::List;
use std::ops::Deref;
use crate::lang::table::{Table, ColumnType, Row};

pub struct SerializationState {
    pub with_id: HashMap<u64, usize>,
    pub values: HashMap<Value, usize>,
}

pub struct DeserializationState {
    pub values: HashMap<usize, Value>,
    pub lists: HashMap<usize, List>,
    pub types: HashMap<usize, ValueType>,
}

pub fn serialize(value: &Value, destination: &Path) -> CrushResult<()> {
    let mut res = SerializedValue::default();
    let mut state = SerializationState {
        with_id: HashMap::new(),
        values: HashMap::new(),
    };
    res.root = value. clone().materialize().serialize(&mut res.elements, &mut state)? as u64;

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
        values: HashMap::new(),
        types: HashMap::new(),
        lists: HashMap::new(),
    };

    let mut res = SerializedValue::decode(&mut Cursor::new(buf)).unwrap();

    println!("AAA {:?}", res);

    Ok(Value::deserialize(res.root as usize, &res.elements, &mut state)?)
}

pub trait Serializable<T> {
    fn deserialize(id: usize, elements: &Vec<Element>, state: &mut DeserializationState) -> CrushResult<T>;
    fn serialize(&self, elements: &mut Vec<Element>, state: &mut SerializationState) -> CrushResult<usize>;
}

impl Serializable<ValueType> for ValueType {
    fn deserialize(id: usize, elements: &Vec<Element>, state: &mut DeserializationState) -> CrushResult<ValueType> {
        if let element::Element::Type(outer_type) = elements[id].element.as_ref().unwrap() {
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
                    Ok(vt)
                }
                _ => unimplemented!(),
            }
        } else {
            error("Invalid type")
        }
    }

    fn serialize(&self, elements: &mut Vec<Element>, state: &mut SerializationState) -> CrushResult<usize> {
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

        let mut node = serialization::Element::default();
        let mut ttt = serialization::Type::default();
        ttt.r#type = Some(SimpleType(tt as i32));
        node.element = Some(element::Element::Type(ttt));
        let idx = elements.len();
        elements.push(node);
        Ok(idx)
    }
}


impl Serializable<Value> for Value {
    fn deserialize(id: usize, elements: &Vec<Element>, state: &mut DeserializationState) -> CrushResult<Value> {
        match elements[id].element.as_ref().unwrap() {
            element::Element::String(s) => {
                Ok(Value::string(s.as_str()))
            }

            element::Element::SmallInteger(i) => {
                Ok(Value::Integer(*i as i128))
            }

            element::Element::Duration(d) => {
                let dd = Duration::seconds(d.secs) + Duration::nanoseconds(d.nanos as i64);
                Ok(Value::Duration(dd))
            }

            element::Element::List(l) =>
                Ok(Value::List(List::deserialize(id, elements, state)?)),

            element::Element::Type(_) =>
                Ok(Value::Type(ValueType::deserialize(id, elements, state)?)),

            element::Element::Table(_) =>
                Ok(Value::Table(Table::deserialize(id, elements, state)?)),
            _ => unimplemented!(),
        }
    }

    fn serialize(&self, elements: &mut Vec<Element>, state: &mut SerializationState) -> CrushResult<usize> {
        if self.value_type().is_hashable() {
            if state.values.contains_key(self) {
                return Ok(state.values[self]);
            }
        }

        match self {
            Value::String(s) => {
                let mut node = Element::default();
                node.element = Some(element::Element::String(s.to_string()));
                let idx = elements.len();
                state.values.insert(self.clone(), idx);
                elements.push(node);
                Ok(idx)
            }
            Value::Integer(s) => {
                let mut node = Element::default();
                match i64::try_from(*s) {
                    Ok(v) => {
                        node.element = Some(element::Element::SmallInteger(v));
                        let idx = elements.len();
                        state.values.insert(self.clone(), idx);
                        elements.push(node);
                        Ok(idx)
                    }
                    Err(_) => {
                        unimplemented!();
                    }
                }
            }
            Value::Duration(d) => {
                let mut node = Element::default();
                let mut dd = serialization::Duration::default();
                dd.secs = d.num_seconds();
                dd.nanos = 0;
                node.element = Some(element::Element::Duration(dd));
                let idx = elements.len();
                state.values.insert(self.clone(), idx);
                elements.push(node);
                Ok(idx)
            }

            Value::Type(t) => t.serialize(elements, state),
            Value::List(l) => l.serialize(elements, state),
            Value::Table(t) => t.serialize(elements, state),

            _ => unimplemented!(),
        }
    }
}

impl Serializable<ColumnType> for ColumnType {
    fn deserialize(id: usize, elements: &Vec<Element>, state: &mut DeserializationState) -> CrushResult<ColumnType> {
        if let element::Element::ColumnType(t) = elements[id].element.as_ref().unwrap(){
            Ok(ColumnType::new(
                t.name.as_str(),
                ValueType::deserialize(t.r#type as usize, elements, state)?))
        } else {
            error("Expected a table")
        }
    }

    fn serialize(&self, elements: &mut Vec<Element>, state: &mut SerializationState) -> CrushResult<usize> {
        let idx = elements.len();
        elements.push(serialization::Element::default());
        let mut stype = serialization::ColumnType::default();
        stype.name = self.name.to_string();
        stype.r#type = self.cell_type.serialize(elements, state)? as u64;
        elements[idx].element = Some(element::Element::ColumnType(stype));
        Ok(idx)
    }
}

impl Serializable<Row> for Row {
    fn deserialize(id: usize, elements: &Vec<Element>, state: &mut DeserializationState) -> CrushResult<Row> {
        if let element::Element::Row(r) = elements[id].element.as_ref().unwrap(){
            let mut cells = Vec::new();
            for c in &r.cells {
                cells.push(Value::deserialize(*c as usize, elements, state)?);
            }
            Ok(Row::new(cells))
        } else {
            error("Expected a table")
        }
    }

    fn serialize(&self, elements: &mut Vec<Element>, state: &mut SerializationState) -> CrushResult<usize> {
        let idx = elements.len();
        elements.push(serialization::Element::default());
        let mut srow = serialization::Row::default();
        for r in self.cells() {
            srow.cells.push(r.serialize(elements, state)? as u64);
        }
        elements[idx].element = Some(element::Element::Row(srow));
        Ok(idx)
    }
}

impl Serializable<Table> for Table {
    fn deserialize(id: usize, elements: &Vec<Element>, state: &mut DeserializationState) -> CrushResult<Table> {
        if let element::Element::Table(lt) = elements[id].element.as_ref().unwrap(){
            let mut column_types = Vec::new();
            let mut rows = Vec::new();
            for ct in &lt.column_types {
                column_types.push(ColumnType::deserialize(*ct as usize, elements, state)?);
            }
            for r in &lt.rows {
                rows.push(Row::deserialize(*r as usize, elements, state)?);
            }
            Ok(Table::new(column_types, rows))
        } else {
            error("Expected a table")
        }
    }

    fn serialize(&self, elements: &mut Vec<Element>, state: &mut SerializationState) -> CrushResult<usize> {
        let idx = elements.len();
        elements.push(serialization::Element::default());
        let mut stable = serialization::Table::default();
        for t in self.types() {
            stable.column_types.push(t.serialize(elements, state)? as u64);
        }
        for r in self.rows() {
            stable.rows.push(r.serialize(elements, state)? as u64);
        }
        elements[idx].element = Some(element::Element::Table(stable));
        Ok(idx)
    }
}

impl Serializable<List> for List {
    fn deserialize(id: usize, elements: &Vec<Element>, state: &mut DeserializationState) -> CrushResult<List> {
        if state.values.contains_key(&id) {
            Ok(state.lists[&id].clone())
        } else {
            if let element::Element::List(l) = elements[id].element.as_ref().unwrap(){
                let element_type = ValueType::deserialize(l.element_type as usize, elements, state)?;
                let list = List::new(element_type, vec![]);
                state.lists.insert(id, list.clone());

                for el_id in &l.elements {
                    list.append(&mut vec![Value::deserialize(*el_id as usize, elements, state)?])?;
                }
                Ok(list)
            } else {
                error("Expected a list")
            }
        }
    }

    fn serialize(&self, elements: &mut Vec<Element>, state: &mut SerializationState) -> CrushResult<usize> {
        let id = self.id();
        if !state.with_id.contains_key(&id) {
            let idx = elements.len();
            elements.push(serialization::Element::default());
            state.with_id.insert(id, idx);

            let type_idx = Value::Type(self.element_type()).serialize(elements, state)?;

            let mut res = Vec::new();
            let data = self.dump();
            res.reserve(data.len());
            let mut ll = serialization::List::default();
            ll.elements = res;
            for el in data {
                ll.elements.push(el.serialize(elements, state)? as u64)
            }
            ll.element_type = type_idx as u64;
            let mut node = serialization::Element::default();
            node.element = Some(element::Element::List(ll));
            elements[idx] = node;
        }
        Ok(state.with_id[&id])
    }
}
