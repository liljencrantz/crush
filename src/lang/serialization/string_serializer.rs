use crate::lang::errors::{CrushResult, error};
use crate::lang::serialization::model::{Element, Strings, element};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};
use crate::lang::value::Value;
use std::collections::hash_map::Entry;

impl Serializable<Vec<String>> for Vec<String> {
    fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<Vec<String>> {
        match elements[id].element.as_ref().unwrap() {
            element::Element::Strings(s) => {
                let mut res = Vec::new();
                for id in s.elements.iter() {
                    let s = String::deserialize(*id as usize, elements, state)?;
                    res.push(s);
                }
                Ok(res)
            }
            _ => error("Expected string list"),
        }
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let ids = self
            .iter()
            .map(|s| s.serialize(elements, state).map(|i| i as u64))
            .collect::<CrushResult<Vec<u64>>>()?;

        let idx = elements.len();
        elements.push(Element {
            element: Some(element::Element::Strings(Strings { elements: ids })),
        });
        Ok(idx)
    }
}

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
        match state.values.entry(Value::from(self)) {
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
