use crate::lang::errors::{CrushResult, error};
use crate::lang::serialization::model;
use crate::lang::serialization::model::scope::ReturnValue::ReturnValueValue;
use crate::lang::serialization::model::{Element, element};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use crate::util::identity_arc::Identity;
use element::Element::Member;
use model::scope::Calling::CallingValue;
use model::scope::Description::{DescriptionValue, HasDescription};
use model::scope::Name::{HasName, NameValue};
use model::scope::Parent::ParentValue;
use std::collections::hash_map::Entry;

impl Serializable<Scope> for Scope {
    fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<Scope> {
        match state.scopes.entry(id) {
            Entry::Occupied(o) => Ok(o.get().clone()),
            Entry::Vacant(_) => match elements[id].element.as_ref().unwrap() {
                element::Element::UserScope(s) => {
                    let name = match s.name {
                        None | Some(HasName(_)) => None,
                        Some(NameValue(n)) => {
                            Some(String::deserialize(n as usize, elements, state)?)
                        }
                    };
                    let description = match s.description {
                        None | Some(HasDescription(_)) => None,
                        Some(DescriptionValue(n)) => {
                            Some(String::deserialize(n as usize, elements, state)?)
                        }
                    };
                    let res = Scope::create(
                        name,
                        description,
                        s.scope_type.try_into()?,
                        s.is_stopped,
                        s.is_readonly,
                    );
                    state.scopes.insert(id, res.clone());
                    if let Some(ParentValue(pid)) = s.parent {
                        res.set_parent(Some(Scope::deserialize(pid as usize, elements, state)?));
                    }
                    if let Some(CallingValue(cid)) = s.calling {
                        res.set_calling(Some(Scope::deserialize(cid as usize, elements, state)?));
                    }
                    for uid in &s.uses {
                        res.r#use(&Scope::deserialize(*uid as usize, elements, state)?);
                    }
                    if let Some(ReturnValueValue(rid)) = s.return_value {
                        res.set_return_value(Value::deserialize(rid as usize, elements, state)?);
                    }

                    for mid in s.members.iter() {
                        match &elements[*mid as usize].element {
                            Some(Member(m)) => {
                                res.redeclare(
                                    &String::deserialize(m.name as usize, elements, state)?,
                                    Value::deserialize(m.value as usize, elements, state)?,
                                )?;
                            }
                            _ => {
                                return error("Invalid scope member");
                            }
                        }
                    }
                    Ok(res)
                }
                element::Element::InternalScope(s) => {
                    let strings = Vec::deserialize(*s as usize, elements, state)?;
                    match state.env.get_absolute_path(strings) {
                        Ok(Value::Scope(s)) => Ok(s),
                        Ok(_) => error("Value is not a scope"),
                        Err(_) => error("Invalid path for scope lookup"),
                    }
                }
                _ => error("Expected a scope"),
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

                match self.full_path() {
                    Ok(p) => {
                        let strings_idx = p.serialize(elements, state)?;
                        elements[idx] = Element {
                            element: Some(element::Element::InternalScope(strings_idx as u64)),
                        };
                    }
                    Err(_) => {
                        let mut sscope: model::Scope = model::Scope::default();
                        let scope_data = self.export()?;

                        sscope.name = match scope_data.name {
                            None => Some(HasName(false)),
                            Some(n) => {
                                Some(NameValue(n.to_string().serialize(elements, state)? as u64))
                            }
                        };
                        sscope.parent = match scope_data.parent_scope {
                            None => Some(model::scope::Parent::HasParent(false)),
                            Some(p) => Some(ParentValue(p.serialize(elements, state)? as u64)),
                        };
                        sscope.calling = match scope_data.calling_scope {
                            None => Some(model::scope::Calling::HasCalling(false)),
                            Some(c) => Some(CallingValue(c.serialize(elements, state)? as u64)),
                        };
                        sscope.is_readonly = scope_data.is_readonly;
                        sscope.scope_type = scope_data.scope_type.into();
                        sscope.is_stopped = scope_data.is_stopped;

                        for u in scope_data.uses.iter() {
                            sscope.uses.push(u.serialize(elements, state)? as u64);
                        }

                        sscope.return_value = match scope_data.return_value {
                            None => None,
                            Some(v) => Some(ReturnValueValue(v.serialize(elements, state)? as u64)),
                        };

                        for (k, v) in scope_data.mapping.iter() {
                            let name_idx = k.to_string().serialize(elements, state)?;
                            let value_idx = v.serialize(elements, state)?;

                            let entry_idx = elements.len();
                            elements.push(Element {
                                element: Some(Member(model::Member {
                                    name: name_idx as u64,
                                    value: value_idx as u64,
                                })),
                            });

                            sscope.members.push(entry_idx as u64);
                        }

                        elements[idx] = Element {
                            element: Some(element::Element::UserScope(sscope)),
                        };
                    }
                }
                Ok(idx)
            }
        }
    }
}
