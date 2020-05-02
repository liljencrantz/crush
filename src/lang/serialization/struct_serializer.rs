use super::super::errors::{error, CrushResult};
use super::super::value::Value;
use super::model;
use super::model::{element, Element};
use super::{DeserializationState, Serializable, SerializationState};
use crate::lang::r#struct::Struct;
use crate::util::identity_arc::Identity;
use std::collections::hash_map::Entry;

impl Serializable<Struct> for Struct {
    fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<Struct> {
        match state.structs.entry(id) {
            Entry::Occupied(o) => Ok(o.get().clone()),
            Entry::Vacant(v) => match elements[id].element.as_ref().unwrap() {
                element::Element::Struct(s) => {
                    let res = Struct::new(vec![], None);
                    v.insert(res.clone());
                    let parent = match s.parent {
                        None | Some(model::r#struct::Parent::HasParent(_)) => None,
                        Some(model::r#struct::Parent::ParentValue(parent_id)) => {
                            Some(Struct::deserialize(parent_id as usize, elements, state)?)
                        }
                    };

                    res.set_parent(parent);

                    for member_idx in &s.members {
                        match elements[*member_idx as usize].element.as_ref().unwrap() {
                            element::Element::Member(smember) => {
                                let name =
                                    String::deserialize(smember.name as usize, elements, state)?;
                                let value =
                                    Value::deserialize(smember.value as usize, elements, state)?;
                                res.set(&name, value);
                            }
                            _ => return error("Expected a member"),
                        }
                    }
                    Ok(res)
                }
                _ => error("Expected struct"),
            },
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

                let mut members = Vec::new();
                for (name, value) in self.local_elements() {
                    let el = model::Element {
                        element: Some(element::Element::Member(model::Member {
                            name: name.to_string().serialize(elements, state)? as u64,
                            value: value.serialize(elements, state)? as u64,
                        })),
                    };
                    members.push(elements.len() as u64);
                    elements.push(el);
                }

                elements[idx] = model::Element {
                    element: Some(element::Element::Struct(model::Struct {
                        parent: None,
                        members,
                    })),
                };

                Ok(idx)
            }
        }
    }
}
