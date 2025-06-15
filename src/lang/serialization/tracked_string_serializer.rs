use crate::lang::errors::{CrushResult, error};
use crate::lang::serialization::model;
use crate::lang::serialization::model::{Element, element};

use crate::lang::ast::location::Location;
use crate::lang::ast::tracked_string::TrackedString;
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};

impl Serializable<TrackedString> for TrackedString {
    fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<TrackedString> {
        match elements[id].element.as_ref().unwrap() {
            element::Element::TrackedString(s) => Ok(TrackedString::new(
                &String::deserialize(s.string as usize, elements, state)?,
                Location::new(s.start as usize, s.end as usize),
            )),
            _ => error("Expected string"),
        }
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let string_id = self.string.serialize(elements, state)?;
        let idx = elements.len();
        elements.push(Element {
            element: Some(element::Element::TrackedString(model::TrackedString {
                start: self.location.start as u64,
                end: self.location.end as u64,
                string: string_id as u64,
            })),
        });
        Ok(idx)
    }
}
