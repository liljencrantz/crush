use super::super::errors::{error, CrushResult};
use crate::lang::data::list::List;
use super::super::value::{Value, ValueType};
use super::model;
use super::model::{element, Element};
use super::{DeserializationState, Serializable, SerializationState};
use crate::util::identity_arc::Identity;
use std::collections::hash_map::Entry;

impl Serializable<List> for List {
    fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<List> {
        match state.lists.entry(id) {
            Entry::Occupied(o) => Ok(o.get().clone()),
            Entry::Vacant(_) => {
                if let element::Element::List(l) = elements[id].element.as_ref().unwrap() {
                    let element_type =
                        ValueType::deserialize(l.element_type as usize, elements, state)?;
                    let list = List::new(element_type, []);
                    state.lists.insert(id, list.clone());

                    for el_id in &l.elements {
                        list.append(&mut vec![Value::deserialize(
                            *el_id as usize,
                            elements,
                            state,
                        )?])?;
                    }
                    Ok(list)
                } else {
                    error("Expected a list")
                }
            }
        }
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let id = self.id();
        match state.with_id.entry(id) {
            Entry::Occupied(o) => Ok(*o.get()),
            Entry::Vacant(v) => {
                let idx = elements.len();
                elements.push(model::Element::default());
                v.insert(idx);

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
                Ok(idx)
            }
        }
    }
}
