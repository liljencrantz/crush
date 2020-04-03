use crate::lang::list::List;
use crate::lang::serialization::{Serializable, DeserializationState, SerializationState};
use crate::lang::errors::{CrushResult, error};
use crate::lang::serialization::model::{Element, element};
use crate::lang::serialization::model;
use crate::lang::value::{ValueType, Value};
use crate::util::identity_arc::Identity;
use chrono::Duration;
use crate::lang::table::Table;
use std::convert::TryFrom;

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

            element::Element::List(l) => Ok(Value::List(List::deserialize(id, elements, state)?)),

            element::Element::Type(_) => Ok(Value::Type(ValueType::deserialize(id, elements, state)?)),

            element::Element::Table(_) => Ok(Value::Table(Table::deserialize(id, elements, state)?)),
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

            _ => unimplemented!(),
        }
    }
}
