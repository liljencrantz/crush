use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;
use std::convert::TryFrom;

pub enum SimpleSignature {
    String,
    Bool,
    I128,
    Usize,
    U64,
    I64,
    U32,
    I32,
    ValueType,
    F64,
    Char,
    Command,
    Duration,
    Struct,
    Stream,
    Value,
    Dict,
    PathBuf,
    Scope,
    BinaryInput,
    Files,
}

impl TryFrom<&Ident> for SimpleSignature {
    type Error = String;

    fn try_from(value: &Ident) -> Result<Self, Self::Error> {
        match value.to_string().as_str() {
            "String" => Ok(SimpleSignature::String),
            "bool" => Ok(SimpleSignature::Bool),
            "i128" => Ok(SimpleSignature::I128),
            "usize" => Ok(SimpleSignature::Usize),
            "u64" => Ok(SimpleSignature::U64),
            "i64" => Ok(SimpleSignature::I64),
            "u32" => Ok(SimpleSignature::U32),
            "i32" => Ok(SimpleSignature::I32),
            "ValueType" => Ok(SimpleSignature::ValueType),
            "f64" => Ok(SimpleSignature::F64),
            "char" => Ok(SimpleSignature::Char),
            "Command" => Ok(SimpleSignature::Command),
            "Duration" => Ok(SimpleSignature::Duration),
            "Struct" => Ok(SimpleSignature::Struct),
            "Stream" => Ok(SimpleSignature::Stream),
            "Value" => Ok(SimpleSignature::Value),
            "Dict" => Ok(SimpleSignature::Dict),
            "PathBuf" => Ok(SimpleSignature::PathBuf),
            "Scope" => Ok(SimpleSignature::Scope),
            "BinaryInput" => Ok(SimpleSignature::BinaryInput),
            "Files" => Ok(SimpleSignature::Files),
            _ => Err("Unknown type".to_string()),
        }
    }
}

impl SimpleSignature {
    pub fn literal(&self) -> Literal {
        Literal::string(self.name())
    }

    pub fn ident(&self, span: Span) -> Ident {
        Ident::new(self.name(), span)
    }

    pub fn name(&self) -> &str {
        match self {
            SimpleSignature::String => "String",
            SimpleSignature::Bool => "bool",
            SimpleSignature::I128 => "i128",
            SimpleSignature::Usize => "usize",
            SimpleSignature::U64 => "u64",
            SimpleSignature::I64 => "i64",
            SimpleSignature::U32 => "u32",
            SimpleSignature::I32 => "i32",
            SimpleSignature::ValueType => "ValueType",
            SimpleSignature::F64 => "f64",
            SimpleSignature::Char => "char",
            SimpleSignature::Command => "Command",
            SimpleSignature::Duration => "Duration",
            SimpleSignature::Struct => "Struct",
            SimpleSignature::Stream => "Stream",
            SimpleSignature::Value => "Value",
            SimpleSignature::Dict => "Dict",
            SimpleSignature::PathBuf => "PathBuf",
            SimpleSignature::Scope => "Scope",
            SimpleSignature::BinaryInput => "BinaryInput",
            SimpleSignature::Files => "Files",
        }
    }

    pub fn value(&self) -> TokenStream {
        match self {
            SimpleSignature::String => quote! {crate::lang::value::Value::String(_value)},
            SimpleSignature::Bool => quote! {crate::lang::value::Value::Bool(_value)},
            SimpleSignature::I128 => quote! {crate::lang::value::Value::Integer(_value)},
            SimpleSignature::Usize => quote! {crate::lang::value::Value::Integer(_value)},
            SimpleSignature::U64 => quote! {crate::lang::value::Value::Integer(_value)},
            SimpleSignature::I64 => quote! {crate::lang::value::Value::Integer(_value)},
            SimpleSignature::U32 => quote! {crate::lang::value::Value::Integer(_value)},
            SimpleSignature::I32 => quote! {crate::lang::value::Value::Integer(_value)},
            SimpleSignature::ValueType => quote! {crate::lang::value::Value::Type(_value)},
            SimpleSignature::F64 => quote! {crate::lang::value::Value::Float(_value)},
            SimpleSignature::Char => quote! {crate::lang::value::Value::String(_value)},
            SimpleSignature::Command => quote! {crate::lang::value::Value::Command(_value)},
            SimpleSignature::Duration => quote! {crate::lang::value::Value::Duration(_value)},
            SimpleSignature::Struct => quote! {crate::lang::value::Value::Struct(_value)},
            SimpleSignature::Dict => quote! {crate::lang::value::Value::Dict(_value)},
            SimpleSignature::Stream => quote! {_value},
            SimpleSignature::Value => quote! {_value},
            SimpleSignature::PathBuf => quote! {crate::lang::value::Value::File(_value)},
            SimpleSignature::Scope => quote! {crate::lang::value::Value::Scope(_value)},
            SimpleSignature::BinaryInput => quote! {_value},
            SimpleSignature::Files => quote! {_value},
        }
    }

    pub fn value_type(&self) -> TokenStream {
        match self {
            SimpleSignature::String => quote! {crate::lang::value::ValueType::String},
            SimpleSignature::Bool => quote! {crate::lang::value::ValueType::Bool},
            SimpleSignature::I128 => quote! {crate::lang::value::ValueType::Integer},
            SimpleSignature::Usize => quote! {crate::lang::value::ValueType::Integer},
            SimpleSignature::U64 => quote! {crate::lang::value::ValueType::Integer},
            SimpleSignature::I64 => quote! {crate::lang::value::ValueType::Integer},
            SimpleSignature::U32 => quote! {crate::lang::value::ValueType::Integer},
            SimpleSignature::I32 => quote! {crate::lang::value::ValueType::Integer},
            SimpleSignature::ValueType => quote! {crate::lang::value::ValueType::Type},
            SimpleSignature::F64 => quote! {crate::lang::value::ValueType::Float},
            SimpleSignature::Char => quote! {crate::lang::value::ValueType::String},
            SimpleSignature::Command => quote! {crate::lang::value::ValueType::Command},
            SimpleSignature::Duration => quote! {crate::lang::value::ValueType::Duration},
            SimpleSignature::Struct => quote! {crate::lang::value::ValueType::Struct},
            SimpleSignature::Dict => quote! {crate::lang::value::ValueType::Struct},
            SimpleSignature::Stream => quote! {crate::lang::value::ValueType::Any},
            SimpleSignature::Value => quote! {crate::lang::value::ValueType::Any},
            SimpleSignature::PathBuf => quote! {crate::lang::value::ValueType::File},
            SimpleSignature::Scope => quote! {crate::lang::value::ValueType::Scope},
            SimpleSignature::BinaryInput => quote! {crate::lang::value::ValueType::OneOf(
                vec![
                    crate::lang::value::ValueType::File,
                    crate::lang::value::ValueType::Glob,
                    crate::lang::value::ValueType::Regex,
                    crate::lang::value::ValueType::String,
                    crate::lang::value::ValueType::Binary,
                    crate::lang::value::ValueType::BinaryInputStream,
                ]
            )},
            SimpleSignature::Files => quote! {crate::lang::value::ValueType::OneOf(
                vec![
                    crate::lang::value::ValueType::File,
                    crate::lang::value::ValueType::Glob,
                    crate::lang::value::ValueType::Regex,
                ]
            )},
        }
    }

    pub fn description(&self) -> &str {
        match self {
            SimpleSignature::String | SimpleSignature::Char => "string",
            SimpleSignature::Bool => "bool",
            SimpleSignature::I128
            | SimpleSignature::Usize
            | SimpleSignature::U64
            | SimpleSignature::I64
            | SimpleSignature::U32
            | SimpleSignature::I32 => "integer",
            SimpleSignature::ValueType => "type",
            SimpleSignature::F64 => "float",
            SimpleSignature::Command => "command",
            SimpleSignature::Duration => "duration",
            SimpleSignature::Value => "any",
            SimpleSignature::Stream => "stream",
            SimpleSignature::Struct => "struct",
            SimpleSignature::Dict => "dict",
            SimpleSignature::PathBuf => "file",
            SimpleSignature::Scope => "scope",
            SimpleSignature::BinaryInput => {
                "one_of $file $string $binary $binary_input_stream $glob $re"
            }
            SimpleSignature::Files => "one_of $file $glob $re",
        }
    }

    pub fn mutator(&self, allowed_values: &Option<Ident>) -> TokenStream {
        match allowed_values {
            None => match self {
                SimpleSignature::Char => {
                    quote! {
                        if _value.len() == 1 {
                            _value.chars().next().unwrap()
                        } else {
                            return crate::lang::errors::argument_error("Argument must be exactly one character", &_source)
                        }
                    }
                }
                SimpleSignature::String => quote! { _value.to_string()},
                SimpleSignature::PathBuf => quote! { _value.to_path_buf()},
                SimpleSignature::Usize => {
                    quote! { crate::lang::errors::with_source(usize::try_from(_value), &_source)? }
                }
                SimpleSignature::U64 => {
                    quote! { crate::lang::errors::with_source(u64::try_from(_value), &_source)?}
                }
                SimpleSignature::I64 => {
                    quote! { crate::lang::errors::with_source(i64::try_from(_value), &_source)?}
                }
                SimpleSignature::U32 => {
                    quote! { crate::lang::errors::with_source(u32::try_from(_value), &_source)?}
                }
                SimpleSignature::I32 => {
                    quote! { crate::lang::errors::with_source(i32::try_from(_value), &_source)?}
                }
                SimpleSignature::Stream => {
                    quote! {
                        // Fixme: Losing location information here!
                        crate::lang::errors::with_source(_value.stream(), &_source)?,
                    }
                }
                SimpleSignature::BinaryInput => {
                    quote! { crate::lang::errors::with_source(crate::lang::signature::binary_input::BinaryInput::try_from(_value), &_source)? }
                }
                SimpleSignature::Files => {
                    quote! { crate::lang::errors::with_source(crate::lang::signature::files::Files::try_from(_value), &_source)? }
                }
                _ => quote! {_value},
            },
            Some(allowed) => match self {
                SimpleSignature::Char => quote! {
                    if _value.len() == 1 {
                        let c = _value.chars().next().unwrap();
                        if #allowed.contains(&c) {
                            c
                        } else {
                            return crate::lang::errors::argument_error(
                                format!("Only the following values are allowed: {:?}", #allowed),
                                &_source,
                            )
                        }
                    } else {
                        return crate::lang::errors::argument_error(
                            "Argument must be exactly one character",
                            &_source,
                        )
                    }
                },
                SimpleSignature::String => quote! {
                    if #allowed.contains(&_value.deref()) {
                        _value.to_string()
                    } else {
                        return crate::lang::errors::argument_error(
                            format!("Only the following values are allowed: {:?}", #allowed),
                            &_source,
                        )
                    }
                },
                _ => quote! {
                    if #allowed.contains(&_value) {
                        _value
                    } else {
                        return crate::lang::errors::argument_error(
                            format!("Only the following values are allowed: {:?}", #allowed),
                            &_source,
                        )
                    }
                },
            },
        }
    }

    pub fn dump_list(&self) -> &str {
        match self {
            SimpleSignature::String => "dump_string",
            SimpleSignature::Bool => "dump_bool",
            SimpleSignature::I128 => "dump_integer",
            SimpleSignature::ValueType => "dump_type",
            SimpleSignature::F64 => "dump_float",
            SimpleSignature::Value => "dump_value",
            SimpleSignature::Dict => "dump_dict",
            SimpleSignature::Scope => "dump_scope",
            SimpleSignature::BinaryInput => "dump_binary_input",
            SimpleSignature::Files => "dump_files",
            _ => panic!("Unknown type"),
        }
    }
}
