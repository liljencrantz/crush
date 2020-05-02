use crate::lang::errors::{error, mandate, CrushResult};
use crate::lang::serialization::model;
use crate::lang::serialization::model::{element, Element};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};
use crate::lang::table::ColumnType;
use crate::lang::value::ValueType;
use model::r#type::SimpleTypeKind;
use model::r#type::Type::SimpleType;

impl Serializable<ValueType> for ValueType {
    fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<ValueType> {
        if let element::Element::Type(outer_type) = elements[id].element.as_ref().unwrap() {
            match mandate(outer_type.r#type.as_ref(), "Missing type")? {
                SimpleType(simple_type) => Ok(match simple_type {
                    0 => ValueType::String,
                    1 => ValueType::Integer,
                    2 => ValueType::File,
                    3 => ValueType::Float,
                    4 => ValueType::Command,
                    5 => ValueType::Binary,
                    6 => ValueType::Duration,
                    7 => ValueType::Field,
                    8 => ValueType::Glob,
                    9 => ValueType::Regex,
                    10 => ValueType::Scope,
                    11 => ValueType::Bool,
                    12 => ValueType::Empty,
                    13 => ValueType::Type,
                    14 => ValueType::Time,
                    15 => ValueType::Struct,
                    16 => ValueType::Any,
                    _ => return error("Unrecognised type"),
                }),
                model::r#type::Type::ListType(l) => Ok(ValueType::List(Box::from(
                    ValueType::deserialize(l.element_type as usize, elements, state)?,
                ))),
                model::r#type::Type::DictType(d) => Ok(ValueType::Dict(
                    Box::from(ValueType::deserialize(
                        d.key_type as usize,
                        elements,
                        state,
                    )?),
                    Box::from(ValueType::deserialize(
                        d.value_type as usize,
                        elements,
                        state,
                    )?),
                )),
                model::r#type::Type::TableType(tt) => Ok(ValueType::Table(
                    tt.column_types
                        .iter()
                        .map(|t| ColumnType::deserialize(*t as usize, elements, state))
                        .collect::<CrushResult<Vec<_>>>()?,
                )),
                model::r#type::Type::TableStreamType(tt) => Ok(ValueType::TableStream(
                    tt.column_types
                        .iter()
                        .map(|t| ColumnType::deserialize(*t as usize, elements, state))
                        .collect::<CrushResult<Vec<_>>>()?,
                )),
            }
        } else {
            error("Invalid type")
        }
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let tt = match self {
            ValueType::String => SimpleTypeKind::String,
            ValueType::Integer => SimpleTypeKind::Integer,
            ValueType::Time => SimpleTypeKind::Time,
            ValueType::Duration => SimpleTypeKind::Duration,
            ValueType::Field => SimpleTypeKind::Field,
            ValueType::Glob => SimpleTypeKind::Glob,
            ValueType::Regex => SimpleTypeKind::Regex,
            ValueType::Command => SimpleTypeKind::Command,
            ValueType::File => SimpleTypeKind::File,
            ValueType::Struct => SimpleTypeKind::Struct,
            ValueType::Scope => SimpleTypeKind::Scope,
            ValueType::Bool => SimpleTypeKind::Bool,
            ValueType::Float => SimpleTypeKind::Float,
            ValueType::Empty => SimpleTypeKind::Empty,
            ValueType::Any => SimpleTypeKind::Any,
            ValueType::Binary => SimpleTypeKind::Binary,
            ValueType::Type => SimpleTypeKind::Type,
            ValueType::List(t) => {
                let l = model::ListType {
                    element_type: t.serialize(elements, state)? as u64,
                };
                let idx = elements.len();
                elements.push(model::Element {
                    element: Some(element::Element::Type(model::Type {
                        r#type: Some(model::r#type::Type::ListType(l)),
                    })),
                });
                return Ok(idx);
            }
            ValueType::Dict(t1, t2) => {
                let d = model::DictType {
                    key_type: t1.serialize(elements, state)? as u64,
                    value_type: t2.serialize(elements, state)? as u64,
                };
                let idx = elements.len();
                elements.push(model::Element {
                    element: Some(element::Element::Type(model::Type {
                        r#type: Some(model::r#type::Type::DictType(d)),
                    })),
                });
                return Ok(idx);
            }
            ValueType::Table(col) => {
                let d = model::TableType {
                    column_types: col
                        .iter()
                        .map(|t| t.serialize(elements, state).map(|c| c as u64))
                        .collect::<CrushResult<Vec<_>>>()?,
                };
                let idx = elements.len();
                elements.push(model::Element {
                    element: Some(element::Element::Type(model::Type {
                        r#type: Some(model::r#type::Type::TableType(d)),
                    })),
                });
                return Ok(idx);
            }
            ValueType::TableStream(col) => {
                let d = model::TableType {
                    column_types: col
                        .iter()
                        .map(|t| t.serialize(elements, state).map(|c| c as u64))
                        .collect::<CrushResult<Vec<_>>>()?,
                };
                let idx = elements.len();
                elements.push(model::Element {
                    element: Some(element::Element::Type(model::Type {
                        r#type: Some(model::r#type::Type::TableStreamType(d)),
                    })),
                });
                return Ok(idx);
            }
            ValueType::BinaryStream => SimpleTypeKind::BinaryStream,
        };

        let idx = elements.len();
        elements.push(model::Element {
            element: Some(element::Element::Type(model::Type {
                r#type: Some(SimpleType(tt as i32)),
            })),
        });
        Ok(idx)
    }
}
