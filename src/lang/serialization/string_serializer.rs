use crate::lang::serialization::{Serializable, DeserializationState, SerializationState};
use crate::lang::serialization::model::{Element, element};
use crate::lang::errors::{CrushResult, error};
use crate::lang::value::Value;

impl Serializable<String> for String {
    fn deserialize(id: usize, elements: &Vec<Element>, _state: &mut DeserializationState) -> CrushResult<String> {
        match elements[id].element.as_ref().unwrap() {
            element::Element::String(s) => Ok(s.clone()),
            _ => error("Expected string"),
        }
    }

    fn serialize(&self, elements: &mut Vec<Element>, state: &mut SerializationState) -> CrushResult<usize> {
        if state.values.contains_key(&Value::string(&self)) {
            return Ok(state.values[&Value::string(&self)])
        }

        let idx = elements.len();
        state.values.insert(Value::string(&self), idx);
        elements.push(Element { element: Some(element::Element::String(self.clone())) });
        Ok(idx)
    }
}
