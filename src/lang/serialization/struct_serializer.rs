use super::super::errors::{CrushResult, error};
use super::super::value::Value;
use super::model;
use super::model::{Element, element};
use super::{DeserializationState, Serializable, SerializationState};
use crate::lang::data::r#struct::Struct;
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
                    let res = Struct::empty(None);
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

                let parent = match self.parent() {
                    Some(p) => Some(model::r#struct::Parent::ParentValue(
                        p.serialize(elements, state)? as u64,
                    )),
                    None => None,
                };

                elements[idx] = model::Element {
                    element: Some(element::Element::Struct(model::Struct {
                        parent,
                        members,
                    })),
                };

                Ok(idx)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::serialization::{deserialize, serialize};
    use crate::lang::state::scope::Scope;

    /// A struct's parent carries its inherited members (e.g. methods declared via
    /// `class()`). serialize() must preserve it so a struct sent through pup (used by
    /// `pup:to`/`pup:from`, `sudo`, and `remote:exec`) still has access to everything it
    /// inherited once deserialized on the other side.
    #[test]
    fn struct_parent_survives_pup_round_trip() {
        let parent = Struct::new(
            vec![("greeting", Value::from("hello from parent"))],
            None,
        );
        let child = Struct::new(vec![("name", Value::from("child"))], Some(parent));

        let mut buf = Vec::new();
        serialize(&Value::Struct(child), &mut buf).unwrap();

        let env = Scope::create_root();
        let restored = deserialize(&buf, &env).unwrap();

        let restored_struct = match restored {
            Value::Struct(s) => s,
            other => panic!("expected a struct, got {}", other.value_type().to_string()),
        };

        // The child's own field must survive the round trip.
        assert!(
            restored_struct.get("name") == Some(Value::from("child")),
            "child's own field `name` did not survive the round trip",
        );
        // The field inherited from the parent must also survive.
        assert!(
            restored_struct.get("greeting") == Some(Value::from("hello from parent")),
            "field `greeting` inherited from the parent did not survive the round trip \
             (the parent link was likely dropped during serialization)",
        );
    }
}
