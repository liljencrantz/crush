use crate::lang::errors::{CrushResult, error};
use crate::lang::serialization::model::{Element, element};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};
use crate::lang::value::Value;
use std::convert::TryFrom;

impl Serializable<i128> for i128 {
    fn deserialize(
        id: usize,
        elements: &[Element],
        _state: &mut DeserializationState,
    ) -> CrushResult<i128> {
        match elements[id].element.as_ref().unwrap() {
            element::Element::SmallInteger(i) => Ok(*i as i128),
            element::Element::LargeInteger(s) => Ok(s.parse()?),
            _ => error("Expected integer"),
        }
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let idx = elements.len();
        state.values.insert(Value::Integer(*self), idx);
        elements.push(Element {
            element: Some(match i64::try_from(*self) {
                Ok(v) => element::Element::SmallInteger(v),
                Err(_) => element::Element::LargeInteger(self.to_string()),
            }),
        });
        Ok(idx)
    }
}
