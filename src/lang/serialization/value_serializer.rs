use crate::lang::command::CrushCommand;
use crate::lang::data::dict::Dict;
use crate::lang::errors::{error, to_crush_error, CrushResult};
use crate::lang::data::list::List;
use crate::lang::data::r#struct::Struct;
use crate::lang::data::scope::Scope;
use crate::lang::serialization::model;
use crate::lang::serialization::model::{element, Element};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};
use crate::lang::data::table::Table;
use crate::lang::value::{Value, ValueType};
use crate::util::glob::Glob;
use chrono::offset::TimeZone;
use chrono::{Duration, Local};
use regex::Regex;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

fn serialize_simple(
    value: &Value,
    elements: &mut Vec<Element>,
    state: &mut SerializationState,
) -> CrushResult<usize> {
    let idx = elements.len();
    state.values.insert(value.clone(), idx);
    let el = Element {
        element: Some(match value {
            Value::String(s) => element::Element::String(s.to_string()),
            Value::Glob(s) => element::Element::Glob(s.to_string()),
            Value::Regex(s, _) => element::Element::Regex(s.to_string()),
            Value::File(b) => element::Element::File(b.as_os_str().to_os_string().into_vec()),
            Value::Binary(b) => element::Element::Binary(b.clone()),
            Value::Float(f) => element::Element::Float(*f),
            Value::Bool(b) => element::Element::Bool(*b),
            Value::Empty() => element::Element::Empty(false),
            Value::Time(d) => element::Element::Time(d.timestamp_nanos()),
            Value::Field(f) => element::Element::Field(f.serialize(elements, state)? as u64),
            _ => return error("Expected simple value"),
        }),
    };
    elements.push(el);
    Ok(idx)
}

impl Serializable<Value> for Value {
    fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<Value> {
        match elements[id].element.as_ref().unwrap() {
            element::Element::String(s) => Ok(Value::string(s.as_str())),
            element::Element::File(f) => Ok(Value::File(PathBuf::from(OsStr::from_bytes(&f[..])))),
            element::Element::Float(v) => Ok(Value::Float(*v)),
            element::Element::Binary(v) => Ok(Value::Binary(v.clone())),
            element::Element::Glob(v) => Ok(Value::Glob(Glob::new(v))),
            element::Element::Regex(v) => {
                Ok(Value::Regex(v.clone(), to_crush_error(Regex::new(v))?))
            }
            element::Element::Bool(v) => Ok(Value::Bool(*v)),
            element::Element::Empty(_) => Ok(Value::Empty()),

            element::Element::SmallInteger(_) | element::Element::LargeInteger(_) => {
                Ok(Value::Integer(i128::deserialize(id, elements, state)?))
            }

            element::Element::Duration(d) => Ok(Value::Duration(
                Duration::seconds(d.secs) + Duration::nanoseconds(d.nanos as i64),
            )),

            element::Element::Time(t) => Ok(Value::Time(Local.timestamp_nanos(*t))),
            element::Element::List(_) => Ok(Value::List(List::deserialize(id, elements, state)?)),
            element::Element::Type(_) => {
                Ok(Value::Type(ValueType::deserialize(id, elements, state)?))
            }
            element::Element::Table(_) => {
                Ok(Value::Table(Table::deserialize(id, elements, state)?))
            }
            element::Element::Struct(_) => {
                Ok(Value::Struct(Struct::deserialize(id, elements, state)?))
            }

            element::Element::Command(_)
            | element::Element::BoundCommand(_)
            | element::Element::Closure(_) => Ok(Value::Command(<dyn CrushCommand>::deserialize(
                id, elements, state,
            )?)),

            element::Element::Field(f) => Ok(Value::Field(Vec::deserialize(*f as usize, elements, state)?)),
            element::Element::UserScope(_) | element::Element::InternalScope(_) => {
                Ok(Value::Scope(Scope::deserialize(id, elements, state)?))
            }
            element::Element::Dict(_) => Ok(Value::Dict(Dict::deserialize(id, elements, state)?)),

            element::Element::TrackedString(_)
            | element::Element::Strings(_)
            | element::Element::ColumnType(_)
            | element::Element::Row(_)
            | element::Element::Member(_) => error("Not a value"),
        }
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        if self.value_type().is_hashable() && state.values.contains_key(self) {
            return Ok(state.values[self]);
        }

        match self {
            Value::String(_)
            | Value::Glob(_)
            | Value::Regex(_, _)
            | Value::File(_)
            | Value::Binary(_)
            | Value::Float(_)
            | Value::Bool(_)
            | Value::Empty()
            | Value::Time(_)
            | Value::Field(_) => serialize_simple(self, elements, state),

            Value::Integer(s) => s.serialize(elements, state),

            Value::Duration(d) => {
                let mut node = Element::default();
                let mut dd = model::Duration::default();
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
            Value::Command(c) => c.serialize(elements, state),
            Value::Struct(s) => s.serialize(elements, state),
            Value::Dict(d) => d.serialize(elements, state),
            Value::Scope(s) => s.serialize(elements, state),
            Value::TableOutputStream(_) | Value::TableInputStream(_) |
            Value::BinaryInputStream(_) => error("Can't serialize streams"),
        }
    }
}
