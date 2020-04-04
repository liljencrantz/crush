use crate::lang::serialization::{Serializable, DeserializationState, SerializationState};
use crate::lang::errors::{CrushResult, error};
use crate::lang::serialization::model::{Element, element};
use crate::lang::serialization::model;
use crate::lang::value::ValueType;
use model::r#type::SimpleTypeKind;
use model::r#type::Type::SimpleType;

impl Serializable<ValueType> for ValueType {
    fn deserialize(id: usize, elements: &Vec<Element>, _state: &mut DeserializationState) -> CrushResult<ValueType> {
        if let element::Element::Type(outer_type) = elements[id].element.as_ref().unwrap() {
            match outer_type.r#type {
                Some(SimpleType(simple_type)) => {
                    let vt = match simple_type {
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
                        _ => return error("Unrecognised type")
                    };
                    Ok(vt)
                }
                _ => unimplemented!(),
            }
        } else {
            error("Invalid type")
        }
    }

    fn serialize(&self, elements: &mut Vec<Element>, _state: &mut SerializationState) -> CrushResult<usize> {
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
            ValueType::List(_) => unimplemented!(),
            ValueType::Dict(_, _) => unimplemented!(),
            ValueType::Table(_) => unimplemented!(),
            ValueType::TableStream(_) => unimplemented!(),
            ValueType::BinaryStream => unimplemented!(),
        };

        let idx = elements.len();
        elements.push(model::Element {
            element: Some(element::Element::Type(
                model::Type {
                    r#type: Some(SimpleType(tt as i32))
                }
            )),
        });
        Ok(idx)
    }
}
