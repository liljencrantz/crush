use std::convert::TryFrom;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;

pub enum SimpleSignature {
    String(Span),
    Bool(Span),
    I128(Span),
    Usize(Span),
    U64(Span),
    I64(Span),
    U32(Span),
    I32(Span),
    ValueType(Span),
    F64(Span),
    Char(Span),
    Command(Span),
    Duration(Span),
    Struct(Span),
    Stream(Span),
    Value(Span),
    Dict(Span),
}

impl TryFrom<&Ident> for SimpleSignature {
    type Error = String;

    fn try_from(value: &Ident) -> Result<Self, Self::Error> {
        match value.to_string().as_str() {
            "String" => Ok(SimpleSignature::String(value.span())),
            "bool" => Ok(SimpleSignature::Bool(value.span())),
            "i128" => Ok(SimpleSignature::I128(value.span())),
            "usize" => Ok(SimpleSignature::Usize(value.span())),
            "u64" => Ok(SimpleSignature::U64(value.span())),
            "i64" => Ok(SimpleSignature::I64(value.span())),
            "u32" => Ok(SimpleSignature::U32(value.span())),
            "i32" => Ok(SimpleSignature::I32(value.span())),
            "ValueType" => Ok(SimpleSignature::ValueType(value.span())),
            "f64" => Ok(SimpleSignature::F64(value.span())),
            "char" => Ok(SimpleSignature::Char(value.span())),
            "Command" => Ok(SimpleSignature::Command(value.span())),
            "Duration" => Ok(SimpleSignature::Duration(value.span())),
            "Struct" => Ok(SimpleSignature::Struct(value.span())),
            "Stream" => Ok(SimpleSignature::Stream(value.span())),
            "Value" => Ok(SimpleSignature::Value(value.span())),
            "Dict" => Ok(SimpleSignature::Dict(value.span())),
            _ => Err("Unknown type".to_string()),
        }
    }
}

impl SimpleSignature {
    pub fn literal(&self) -> Literal {
        Literal::string(self.name())
    }

    pub fn span(&self) -> Span {
        match self {
            SimpleSignature::String(s) => *s,
            SimpleSignature::Bool(s) => *s,
            SimpleSignature::I128(s) => *s,
            SimpleSignature::Usize(s) => *s,
            SimpleSignature::U64(s) => *s,
            SimpleSignature::I64(s) => *s,
            SimpleSignature::U32(s) => *s,
            SimpleSignature::I32(s) => *s,
            SimpleSignature::ValueType(s) => *s,
            SimpleSignature::F64(s) => *s,
            SimpleSignature::Char(s) => *s,
            SimpleSignature::Command(s) => *s,
            SimpleSignature::Duration(s) => *s,
            SimpleSignature::Struct(s) => *s,
            SimpleSignature::Stream(s) => *s,
            SimpleSignature::Value(s) => *s,
            SimpleSignature::Dict(s) => *s,
        }
    }

    pub fn ident(&self) -> Ident {
        Ident::new(self.name(), self.span())
    }

    pub fn name(&self) -> &str {
        match self {
            SimpleSignature::String(_) => "String",
            SimpleSignature::Bool(_) => "bool",
            SimpleSignature::I128(_) => "i128",
            SimpleSignature::Usize(_) => "usize",
            SimpleSignature::U64(_) => "u64",
            SimpleSignature::I64(_) => "i64",
            SimpleSignature::U32(_) => "u32",
            SimpleSignature::I32(_) => "i32",
            SimpleSignature::ValueType(_) => "ValueType",
            SimpleSignature::F64(_) => "f64",
            SimpleSignature::Char(_) => "char",
            SimpleSignature::Command(_) => "Command",
            SimpleSignature::Duration(_) => "Duration",
            SimpleSignature::Struct(_) => "Struct",
            SimpleSignature::Stream(_) => "Stream",
            SimpleSignature::Value(_) => "Value",
            SimpleSignature::Dict(_) => "Dict",
        }
    }

    pub fn value(&self) -> TokenStream {
        match self {
            SimpleSignature::String(_) => quote! {crate::lang::value::Value::String(_value)},
            SimpleSignature::Bool(_) => quote! {crate::lang::value::Value::Bool(_value)},
            SimpleSignature::I128(_) => quote! {crate::lang::value::Value::Integer(_value)},
            SimpleSignature::Usize(_) => quote! {crate::lang::value::Value::Integer(_value)},
            SimpleSignature::U64(_) => quote! {crate::lang::value::Value::Integer(_value)},
            SimpleSignature::I64(_) => quote! {crate::lang::value::Value::Integer(_value)},
            SimpleSignature::U32(_) => quote! {crate::lang::value::Value::Integer(_value)},
            SimpleSignature::I32(_) => quote! {crate::lang::value::Value::Integer(_value)},
            SimpleSignature::ValueType(_) => quote! {crate::lang::value::Value::Type(_value)},
            SimpleSignature::F64(_) => quote! {crate::lang::value::Value::Float(_value)},
            SimpleSignature::Char(_) => quote! {crate::lang::value::Value::String(_value)},
            SimpleSignature::Command(_) => quote! {crate::lang::value::Value::Command(_value)},
            SimpleSignature::Duration(_) => quote! {crate::lang::value::Value::Duration(_value)},
            SimpleSignature::Struct(_) => quote! {crate::lang::value::Value::Struct(_value)},
            SimpleSignature::Dict(_) => quote! {crate::lang::value::Value::Dict(_value)},
            SimpleSignature::Stream(_) => quote! {_value},
            SimpleSignature::Value(_) => quote! {_value},
        }
    }

    pub fn value_type(&self) -> TokenStream {
        match self {
            SimpleSignature::String(_) => quote! {crate::lang::value::ValueType::String},
            SimpleSignature::Bool(_) => quote! {crate::lang::value::ValueType::Bool},
            SimpleSignature::I128(_) => quote! {crate::lang::value::ValueType::Integer},
            SimpleSignature::Usize(_) => quote! {crate::lang::value::ValueType::Integer},
            SimpleSignature::U64(_) => quote! {crate::lang::value::ValueType::Integer},
            SimpleSignature::I64(_) => quote! {crate::lang::value::ValueType::Integer},
            SimpleSignature::U32(_) => quote! {crate::lang::value::ValueType::Integer},
            SimpleSignature::I32(_) => quote! {crate::lang::value::ValueType::Integer},
            SimpleSignature::ValueType(_) => quote! {crate::lang::value::ValueType::Type},
            SimpleSignature::F64(_) => quote! {crate::lang::value::ValueType::Float},
            SimpleSignature::Char(_) => quote! {crate::lang::value::ValueType::String},
            SimpleSignature::Command(_) => quote! {crate::lang::value::ValueType::Command},
            SimpleSignature::Duration(_) => quote! {crate::lang::value::ValueType::Duration},
            SimpleSignature::Struct(_) => quote! {crate::lang::value::ValueType::Struct},
            SimpleSignature::Dict(_) => quote! {crate::lang::value::ValueType::Struct},
            SimpleSignature::Stream(_) => quote! {crate::lang::value::ValueType::Any},
            SimpleSignature::Value(_) => quote! {crate::lang::value::ValueType::Any},
        }
    }

    pub fn description(&self) -> &str {
        match self {
            SimpleSignature::String(_) | SimpleSignature::Char(_) => "string",
            SimpleSignature::Bool(_) => "bool",
            SimpleSignature::I128(_) | SimpleSignature::Usize(_) | SimpleSignature::U64(_) |
            SimpleSignature::I64(_) | SimpleSignature::U32(_) | SimpleSignature::I32(_) => "integer",
            SimpleSignature::ValueType(_) => "type",
            SimpleSignature::F64(_) => "float",
            SimpleSignature::Command(_) => "command",
            SimpleSignature::Duration(_) => "duration",
            SimpleSignature::Value(_) => "any value",
            SimpleSignature::Stream(_) => "stream",
            SimpleSignature::Struct(_) => "struct",
            SimpleSignature::Dict(_) => "dict",
        }
    }

    pub fn mutator(&self, allowed_values: &Option<Ident>) -> TokenStream {
        match allowed_values {
            None => match self {
                SimpleSignature::Char(_) => {
                    quote! {
                    if _value.len() == 1 {
                        _value.chars().next().unwrap()
                    } else {
                        return crate::lang::errors::argument_error("Argument must be exactly one character", _location)
                    }
                }
                }
                SimpleSignature::Usize(_) => quote! { crate::lang::errors::to_crush_error(usize::try_from(_value))?},
                SimpleSignature::U64(_) => quote! { crate::lang::errors::to_crush_error(u64::try_from(_value))?},
                SimpleSignature::I64(_) => quote! { crate::lang::errors::to_crush_error(i64::try_from(_value))?},
                SimpleSignature::U32(_) => quote! { crate::lang::errors::to_crush_error(u32::try_from(_value))?},
                SimpleSignature::I32(_) => quote! { crate::lang::errors::to_crush_error(i32::try_from(_value))?},
                SimpleSignature::Stream(_) => {
                    quote! {
                    crate::lang::errors::mandate_argument(
                        _value.stream()?,
                        "Expected a type that can be streamed",
                        _location)?,
                    }
                }
                _ => quote! {_value},
            },
            Some(allowed) => match self {
                SimpleSignature::Char(_) => quote! {
                if _value.len() == 1 {
                    let c = _value.chars().next().unwrap();
                    if #allowed.contains(&c) {
                        c
                    } else {
                        return crate::lang::errors::argument_error(
                            format!("Only the following values are allowed: {:?}", #allowed),
                            _location,
                        )
                    }
                } else {
                    return crate::lang::errors::argument_error(
                        "Argument must be exactly one character",
                        _location,
                    )
                }
            },
                SimpleSignature::String(_) => quote! {
                if #allowed.contains(&_value.as_str()) {
                    _value
                } else {
                    return crate::lang::errors::argument_error(
                        format!("Only the following values are allowed: {:?}", #allowed),
                        _location,
                    )
                }
            },
                _ => quote! {
                if #allowed.contains(&_value) {
                    _value
                } else {
                    return crate::lang::errors::argument_error(
                        format!("Only the following values are allowed: {:?}", #allowed),
                        _location,
                    )
                }
            },
            },
        }
    }

    pub fn dump_list(&self) -> &str {
        match self {
            SimpleSignature::String(_) => "dump_string",
            SimpleSignature::Bool(_) => "dump_bool",
            SimpleSignature::I128(_) => "dump_integer",
            SimpleSignature::ValueType(_) => "dump_type",
            SimpleSignature::F64(_) => "dump_float",
            SimpleSignature::Value(_) => "dump_value",
            SimpleSignature::Dict(_) => "dump_dict",
            _ => panic!("Unknown type"),
        }
    }
}
