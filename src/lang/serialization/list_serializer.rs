use super::super::list::List;
use super::{Serializable, DeserializationState, SerializationState};
use super::super::errors::{CrushResult, error};
use super::model::{Element, element};
use super::model;
use super::super::value::{ValueType, Value};
use crate::util::identity_arc::Identity;

impl Serializable<List> for List {
    fn deserialize(id: usize, elements: &Vec<Element>, state: &mut DeserializationState) -> CrushResult<List> {
        if state.lists.contains_key(&id) {
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
            elements.push(model::Element::default());
            state.with_id.insert(id, idx);

            let type_idx = Value::Type(self.element_type()).serialize(elements, state)?;
            let mut ll = model::List::default();
            ll.elements.reserve(self.len());
            for el in self.dump() {
                ll.elements.push(el.serialize(elements, state)? as u64)
            }
            ll.element_type = type_idx as u64;
            elements[idx] = model::Element {
                element: Some(element::Element::List(ll)),
            };
        }
        Ok(state.with_id[&id])
    }
}
