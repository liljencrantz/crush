use crate::lang::errors::{error, CrushResult};
use crate::lang::serialization::model::{element, Element};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};
use crate::lang::value::Value;
use std::collections::hash_map::Entry;

impl Serializable<String> for String {
    fn deserialize(
        id: usize,
        elements: &[Element],
        _state: &mut DeserializationState,
    ) -> CrushResult<String> {
        match elements[id].element.as_ref().unwrap() {
            element::Element::String(s) => Ok(s.clone()),
            _ => error("Expected string"),
        }
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        match state.values.entry(Value::string(&self)) {
            Entry::Occupied(o) => Ok(*o.get()),
            Entry::Vacant(v) => {
                let idx = elements.len();
                v.insert(idx);
                elements.push(Element {
                    element: Some(element::Element::String(self.clone())),
                });
                Ok(idx)
            }
        }
    }
}
