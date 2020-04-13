use crate::lang::r#struct::Struct;
use super::{Serializable, DeserializationState, SerializationState};
use super::super::errors::{CrushResult, error};
use super::model::{Element, element};
use super::model;
use super::super::value::{Value};
use crate::util::identity_arc::Identity;

impl Serializable<Struct> for Struct {
    fn deserialize(id: usize, elements: &Vec<Element>, state: &mut DeserializationState) -> CrushResult<Struct> {
        if state.structs.contains_key(&id) {
            Ok(state.structs[&id].clone())
        } else {
            match elements[id].element.as_ref().unwrap() {
                element::Element::Struct(s) => {
                    let res = Struct::new(vec![], None);
                    state.structs.insert(id, res.clone());
                    let parent = match s.parent {
                        None | Some(model::r#struct::Parent::HasParent(_)) => None,
                        Some(model::r#struct::Parent::ParentValue(parent_id)) => Some(Struct::deserialize(parent_id as usize, elements, state)?),
                    };

                    res.set_parent(parent);

                    for member_idx in  &s.members {
                        match elements[*member_idx as usize].element.as_ref().unwrap() {
                            element::Element::Member(smember) => {
                                let name = String::deserialize(smember.name as usize, elements, state)?;
                                let value = Value::deserialize(smember.value as usize, elements, state)?;
                                res.set(&name, value);
                            }
                            _ => return error("Expected a member"),
                        }
                    }
                    Ok(res)
                },
                _ => error("Expected struct"),
            }
        }
    }

    fn serialize(&self, elements: &mut Vec<Element>, state: &mut SerializationState) -> CrushResult<usize> {
        let id = self.id();
        if !state.with_id.contains_key(&id) {
            let idx = elements.len();
            elements.push(model::Element::default());
            state.with_id.insert(id, idx);

            let mut members = Vec::new();
            for (name, value) in self.local_elements() {
                let el = model::Element {
                    element: Some(element::Element::Member(model::Member {
                        name: name.to_string().serialize(elements, state)? as u64,
                        value: value.serialize(elements, state)? as u64,
                    }))};
                members.push(elements.len() as u64);
                elements.push(el);
            }

            elements[idx] = model::Element {
                element: Some(element::Element::Struct(model::Struct {
                    parent: None,
                    members,
                })),
            };
        }
        Ok(state.with_id[&id])
    }
}
