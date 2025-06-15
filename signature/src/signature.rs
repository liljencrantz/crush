use crate::{SignatureResult, SimpleSignature};
use proc_macro2::{Ident, Literal, Span, TokenStream, TokenTree};
use quote::{quote, quote_spanned};
use std::convert::TryFrom;
use syn::spanned::Spanned;
use syn::{GenericArgument, PathArguments, Type};

macro_rules! fail {
    ($span:expr, $msg:literal) => {
        Err(quote_spanned! {$span => compile_error!($msg);})
    };
}

pub struct TypeData {
    pub initialize: TokenStream,
    pub mappings: TokenStream,
    pub unnamed_mutate: Option<TokenStream>,
    pub assign: TokenStream,
    pub crush_internal_type: TokenStream,
    pub signature: String,
    pub allowed_values: Option<Vec<TokenTree>>,
}

pub enum SignatureType {
    Simple(SimpleSignature),
    Vec(SimpleSignature),
    Option(SimpleSignature),
    Patterns,
    OrderedStringMap(SimpleSignature),
    Files,
    Number,
    Text,
}

pub struct Signature {
    span: Span,
    signature_type: SignatureType,
    name: Ident,
    default: Option<TokenTree>,
    is_unnamed_target: bool,
    allowed_values: Option<Vec<TokenTree>>,
}

impl Signature {
    pub fn new(
        ty: &Type,
        name: &Ident,
        default: Option<TokenTree>,
        is_unnamed_target: bool,
        allowed_values: Option<Vec<TokenTree>>,
    ) -> SignatureResult<Signature> {
        let signature_type = SignatureType::try_from(ty)?;
        Ok(Signature {
            span: ty.span(),
            signature_type,
            name: name.clone(),
            default,
            is_unnamed_target,
            allowed_values,
        })
    }

    pub fn type_data(self) -> SignatureResult<TypeData> {
        match &self.signature_type {
            SignatureType::Simple(simple_type) => simple_type_data(
                simple_type,
                &self.name,
                self.default,
                self.is_unnamed_target,
                self.allowed_values,
                self.span,
            ),
            SignatureType::Vec(sub) => vec_type_data(
                sub,
                &self.name,
                self.default,
                self.is_unnamed_target,
                self.allowed_values,
                self.span,
            ),
            SignatureType::Option(sub) => option_type_data(
                sub,
                &self.name,
                self.default,
                self.is_unnamed_target,
                self.allowed_values,
                self.span,
            ),
            SignatureType::Patterns => patterns_type_data(
                &self.name,
                self.default,
                self.is_unnamed_target,
                self.allowed_values,
                self.span,
            ),
            SignatureType::OrderedStringMap(sub) => ordered_string_map_type_data(
                sub,
                &self.name,
                self.default,
                self.is_unnamed_target,
                self.allowed_values,
                self.span,
            ),
            SignatureType::Files => files_type_data(
                &self.name,
                self.default,
                self.is_unnamed_target,
                self.allowed_values,
                self.span,
            ),
            SignatureType::Number => number_type_data(
                &self.name,
                self.default,
                self.is_unnamed_target,
                self.allowed_values,
                self.span,
            ),
            SignatureType::Text => text_type_data(
                &self.name,
                self.default,
                self.is_unnamed_target,
                self.allowed_values,
                self.span,
            ),
        }
    }
}

impl TryFrom<&Type> for SignatureType {
    type Error = TokenStream;

    fn try_from(ty: &Type) -> SignatureResult<Self> {
        match ty {
            Type::Path(path) => {
                match (
                    &path.qself,
                    &path.path.leading_colon,
                    path.path.segments.len(),
                ) {
                    (None, None, 1) => {
                        let seg = &path.path.segments[0];
                        let name = seg.ident.to_string();
                        let mut arguments = extract_argument(&seg.arguments)?;

                        if let Ok(simple) = SimpleSignature::try_from(&seg.ident) {
                            if arguments.len() != 0 {
                                return fail!(ty.span(), "Unexpected generic arguments");
                            }
                            return Ok(SignatureType::Simple(simple));
                        } else {
                            match name.as_str() {
                                "Vec" => {
                                    if arguments.len() != 1 {
                                        fail!(ty.span(), "Expected one generic argument")
                                    } else {
                                        Ok(SignatureType::Vec(arguments.remove(0)))
                                    }
                                }
                                "Option" => {
                                    if arguments.len() != 1 {
                                        fail!(ty.span(), "Expected one generic argument")
                                    } else {
                                        Ok(SignatureType::Option(arguments.remove(0)))
                                    }
                                }
                                "OrderedStringMap" => {
                                    if arguments.len() != 1 {
                                        fail!(ty.span(), "Expected one generic argument")
                                    } else {
                                        Ok(SignatureType::OrderedStringMap(arguments.remove(0)))
                                    }
                                }
                                "Patterns" => {
                                    if arguments.len() != 0 {
                                        fail!(ty.span(), "Unexopected generic argument")
                                    } else {
                                        Ok(SignatureType::Patterns)
                                    }
                                }
                                "Files" => {
                                    if arguments.len() != 0 {
                                        fail!(ty.span(), "Unexopected generic argument")
                                    } else {
                                        Ok(SignatureType::Files)
                                    }
                                }
                                "Number" => {
                                    if arguments.len() != 0 {
                                        fail!(ty.span(), "Unexopected generic argument")
                                    } else {
                                        Ok(SignatureType::Number)
                                    }
                                }
                                "Text" => {
                                    if arguments.len() != 0 {
                                        fail!(ty.span(), "Unexopected generic argument")
                                    } else {
                                        Ok(SignatureType::Text)
                                    }
                                }
                                _ => fail!(ty.span(), "Unknown argument type"),
                            }
                        }
                    }
                    _ => fail!(ty.span(), "Invalid type"),
                }
            }
            _ => fail!(ty.span(), "Invalid type, expected a Path segment"),
        }
    }
}

fn extract_argument(path: &PathArguments) -> SignatureResult<Vec<SimpleSignature>> {
    match path {
        PathArguments::None => Ok(Vec::new()),
        PathArguments::AngleBracketed(a) => {
            let mut res = Vec::new();
            for g in &a.args {
                match g {
                    GenericArgument::Type(t) => match SignatureType::try_from(t) {
                        Ok(SignatureType::Simple(s)) => res.push(s),
                        _ => return fail!(path.span(), "Expected a simple type"),
                    },
                    _ => return fail!(path.span(), "Expected a type"),
                }
            }
            Ok(res)
        }
        PathArguments::Parenthesized(_) => Ok(Vec::new()),
    }
}

fn allowed_values_name(
    allowed_values: &Option<Vec<TokenTree>>,
    name: &str,
    span: Span,
) -> Option<Ident> {
    allowed_values.as_ref().map(|_| {
        Ident::new(
            &format!("_{}_allowed_values", name.to_string()),
            span.clone(),
        )
    })
}

fn simple_type_data(
    simple_type: &SimpleSignature,
    name: &Ident,
    default: Option<TokenTree>,
    _is_unnamed_target: bool,
    allowed_values: Option<Vec<TokenTree>>,
    span: Span,
) -> SignatureResult<TypeData> {
    let native_type = simple_type.ident(span);
    let allowed_values_name = allowed_values_name(&allowed_values, &name.to_string(), span);
    let mutator = simple_type.mutator(&allowed_values_name);
    let value_type = simple_type.value();
    let name_literal = Literal::string(&name.to_string());
    let type_name = simple_type.name();

    Ok(TypeData {
        crush_internal_type: simple_type.value_type(),
        signature: if default.is_none() {
            format!(
                "{}={}",
                name.to_string(),
                simple_type.description().to_string().to_lowercase()
            )
        } else {
            if simple_type.description() == "bool"
                && default.is_some()
                && default.as_ref().unwrap().to_string() == "(false)"
            {
                format!("[--{}]", name)
            } else {
                format!("[{}={}]", name.to_string(), simple_type.description())
            }
        },
        initialize: match &allowed_values {
            None => quote! { let mut #name = None; },
            Some(literals) => {
                let mut literal_params = proc_macro2::TokenStream::new();
                for l in literals {
                    literal_params.extend(quote! { #l,});
                }
                quote! {
                    let mut #name = None;
                    let #allowed_values_name = maplit::hashset![#literal_params];
                }
            }
        },
        allowed_values,
        mappings: quote! {(Some(#name_literal), #value_type) => #name = Some(#mutator),},
        unnamed_mutate: match default {
            None => Some(quote! {
            if #name.is_none() {
                match _unnamed.pop_front() {
                    Some((#value_type, _location)) => #name = Some(#mutator),
                    Some((value, _location)) =>
                        return crate::lang::errors::argument_error(format!(
                            "Expected argument \"{}\" to be of type {}, was of type {}",
                            #name_literal,
                            #type_name,
                            value.value_type().to_string()),
                            _location,
                        ),
                    _ =>
                        return crate::lang::errors::argument_error_legacy(
                            format!(
                                "No value provided for argument \"{}\"",
                                #name_literal),
                        ),
                }
            }
                                        }),
            Some(def) => Some(quote! {
            if #name.is_none() {
                match _unnamed.pop_front() {
                    Some((#value_type, _location)) => #name = Some(#mutator),
                    None => #name = Some(#native_type::from(#def)),
                    Some((_, _location)) => return crate::lang::errors::argument_error(
                            format!("Expected argument {} to be of type {}", #name_literal, #type_name),
                            _location,
                        ),
                    _ => return crate::lang::errors::argument_error_legacy(
                            format!("Expected argument {} to be of type {}", #name_literal, #type_name),
                        ),
                    }
            }
                                        }),
        },
        assign: quote! {
        #name: #name.ok_or(format!("Missing value for parameter {}", #name_literal).as_str())?,
        },
    })
}

fn number_type_data(
    name: &Ident,
    default: Option<TokenTree>,
    _is_unnamed_target: bool,
    _allowed_values: Option<Vec<TokenTree>>,
    _span: Span,
) -> SignatureResult<TypeData> {
    let name_literal = proc_macro2::Literal::string(&name.to_string());
    Ok(TypeData {
        allowed_values: None,
        crush_internal_type: quote! {crate::lang::value::ValueType::either(vec![
            crate::lang::value::ValueType::Integer,
            crate::lang::value::ValueType::Float,
        ])},
        signature: format!("{}=(float|integer)", name.to_string()),
        initialize: quote! { let mut #name = None; },
        mappings: quote! {
            (Some(#name_literal), crate::lang::value::Value::Float(_value)) => #name = Some(Number::Float(_value)),
            (Some(#name_literal), crate::lang::value::Value::Integer(_value)) => #name = Some(Number::Integer(_value)),
        },
        unnamed_mutate: if default.is_none() {
            Some(quote! {
                if # name.is_none() {
                    match _unnamed.pop_front() {
                        Some(( crate::lang::value::Value::Float(_value), _location)) => # name = Some(Number::Float(_value)),
                        Some(( crate::lang::value::Value::Integer(_value), _location)) => # name = Some(Number::Integer(_value)),
                        Some((value, _location)) =>
                            return crate::lang::errors::argument_error(format ! (
                                "Expected argument \"{}\" to be a number, was of type {}",
                                #name_literal,
                                value.value_type().to_string()),
                                _location),
                        _ =>
                            return crate::lang::errors::argument_error_legacy(format ! (
                                "No value provided for argument \"{}\"",
                                # name_literal).as_str()),
                    }
                }
            })
        } else {
            Some(quote! {
                if # name.is_none() {
                    match _unnamed.pop_front() {
                        Some(( crate::lang::value::Value::Float(_value), _location)) => # name = Some(Number::Float(_value)),
                        Some(( crate::lang::value::Value::Integer(_value), _location)) => # name = Some(Number::Integer(_value)),
                        Some((value, _location)) =>
                            return crate::lang::errors::argument_error(format ! (
                                "Expected argument \"{}\" to be a number, was of type {}",
                                #name_literal,
                                value.value_type().to_string()),
                                _location),
                        _ => {}
                    }
                }
            })
        },
        assign: default
            .map(|default| {
                quote! {
                    # name: # name.unwrap_or( # default),
                }
            })
            .unwrap_or(quote! {
            #name:
                #name.ok_or(format!("Missing value for parameter {}", #name_literal).as_str())?,
            }),
    })
}

fn text_type_data(
    name: &Ident,
    default: Option<TokenTree>,
    _is_unnamed_target: bool,
    _allowed_values: Option<Vec<TokenTree>>,
    _span: Span,
) -> SignatureResult<TypeData> {
    let name_literal = proc_macro2::Literal::string(&name.to_string());
    Ok(TypeData {
        allowed_values: None,
        crush_internal_type: quote! {crate::lang::value::ValueType::either(vec![
            crate::lang::value::ValueType::String,
            crate::lang::value::ValueType::File,
        ])},
        signature: format!("{}=(string|file)", name.to_string()),
        initialize: quote! { let mut #name = None; },
        mappings: quote! {
            (Some(#name_literal), crate::lang::value::Value::String(_value)) => #name = Some(Text::String(_value)),
            (Some(#name_literal), crate::lang::value::Value::File(_value)) => #name = Some(Text::File(_value)),
        },
        unnamed_mutate: if default.is_none() {
            Some(quote! {
                if # name.is_none() {
                    match _unnamed.pop_front() {
                        Some(( crate::lang::value::Value::String(_value), _location)) => # name = Some(Text::String(_value)),
                        Some(( crate::lang::value::Value::File(_value), _location)) => # name = Some(Text::File(_value)),
                        Some((value, _location)) =>
                            return crate::lang::errors::argument_error(format ! (
                                "Expected argument \"{}\" to be textual, was of type {}",
                                #name_literal,
                                value.value_type().to_string()),
                                _location),
                        _ =>
                            return crate::lang::errors::argument_error_legacy(format ! (
                                "No value provided for argument \"{}\"",
                                # name_literal).as_str()),
                    }
                }
            })
        } else {
            Some(quote! {
                if # name.is_none() {
                    match _unnamed.pop_front() {
                        Some(( crate::lang::value::Value::String(_value), _location)) => # name = Some(Text::String(_value)),
                        Some(( crate::lang::value::Value::File(_value), _location)) => # name = Some(Text::File(_value)),
                        Some((value, _location)) =>
                            return crate::lang::errors::argument_error(format ! (
                                "Expected argument \"{}\" to be textual, was of type {}",
                                #name_literal,
                                value.value_type().to_string()),
                                _location),
                        _ => {}
                    }
                }
            })
        },
        assign: default
            .map(|default| {
                quote! {
                    # name: # name.unwrap_or( # default),
                }
            })
            .unwrap_or(quote! {
            #name: #name.ok_or(format!("Missing value for parameter {}", #name_literal).as_str())?,
            }),
    })
}

fn files_type_data(
    name: &Ident,
    _default: Option<TokenTree>,
    is_unnamed_target: bool,
    _allowed_values: Option<Vec<TokenTree>>,
    _span: Span,
) -> SignatureResult<TypeData> {
    let name_literal = proc_macro2::Literal::string(&name.to_string());
    Ok(TypeData {
        allowed_values: None,
        signature: format!(
            "[{}=(file|glob|regex|list|table|table_input_stream)...]",
            name.to_string()
        ),
        initialize: quote! { let mut #name = crate::lang::signature::files::Files::new(); },
        mappings: quote! { (Some(#name_literal), value) => #name.expand(value)?, },
        unnamed_mutate: if is_unnamed_target {
            Some(quote! {
                while !_unnamed.is_empty() {
                    #name.expand(_unnamed.pop_front().unwrap().0)?;
                }
            })
        } else {
            None
        },
        assign: quote! { #name, },
        crush_internal_type: quote! {crate::lang::value::ValueType::Any},
    })
}

fn patterns_type_data(
    name: &Ident,
    _default: Option<TokenTree>,
    is_unnamed_target: bool,
    _allowed_values: Option<Vec<TokenTree>>,
    _span: Span,
) -> SignatureResult<TypeData> {
    let name_literal = proc_macro2::Literal::string(&name.to_string());
    Ok(TypeData {
        allowed_values: None,
        signature: format!("[{}=(string|glob|regex)...]", name.to_string()),
        initialize: quote! { let mut #name = crate::lang::signature::patterns::Patterns::new(); },
        mappings: quote! {
            (Some(#name_literal), crate::lang::value::Value::Glob(value)) => #name.expand_glob(value),
            (Some(#name_literal), crate::lang::value::Value::String(value)) => #name.expand_string(value.to_string()),
            (Some(#name_literal), crate::lang::value::Value::Regex(pattern, value)) => #name.expand_regex(pattern, value),
        },
        unnamed_mutate: if is_unnamed_target {
            Some(quote! {
                while !_unnamed.is_empty() {
                    match _unnamed.pop_front().unwrap().0 {
            crate::lang::value::Value::Glob(value) => #name.expand_glob(value),
            crate::lang::value::Value::String(value) => #name.expand_string(value.to_string()),
            crate::lang::value::Value::Regex(pattern, value) => #name.expand_regex(pattern, value),
                    }
                }
            })
        } else {
            None
        },
        assign: quote! { #name, },
        crush_internal_type: quote! {crate::lang::value::ValueType::either(vec![
            crate::lang::value::ValueType::String,
            crate::lang::value::ValueType::Glob,
            crate::lang::value::ValueType::Regex,
        ])},
    })
}

fn option_type_data(
    simple_type: &SimpleSignature,
    name: &Ident,
    _default: Option<TokenTree>,
    _is_unnamed_target: bool,
    _allowed_values: Option<Vec<TokenTree>>,
    span: Span,
) -> SignatureResult<TypeData> {
    let sub_type = simple_type.literal();
    let mutator = simple_type.mutator(&None);
    let value_type = simple_type.value();
    let name_literal = proc_macro2::Literal::string(&name.to_string());

    Ok(TypeData {
        allowed_values: None,
        signature: format!(
            "[{}={}]",
            name.to_string(),
            simple_type.description().to_string().to_lowercase()
        ),
        initialize: quote! { let mut #name = None; },
        mappings: quote! { (Some(#name_literal), #value_type) => #name = Some(#mutator), },
        unnamed_mutate: Some(quote_spanned! { span =>
        if #name.is_none() {
            match _unnamed.pop_front() {
                None => {}
                Some((#value_type, _location)) => #name = Some(#mutator),
                Some((_, _location)) =>
                    return crate::lang::errors::argument_error(
                        format!("Expected argument {} to be of type {}", #name_literal, #sub_type),
                        _location,
                    ),
                _ =>
                    return crate::lang::errors::argument_error_legacy(
                        format!("Missing argument {}", #name_literal)),
            }
        }
        }),
        assign: quote! { #name, },
        crush_internal_type: simple_type.value_type(),
    })
}

fn ordered_string_map_type_data(
    simple_type: &SimpleSignature,
    name: &Ident,
    _default: Option<TokenTree>,
    _is_unnamed_target: bool,
    allowed_values: Option<Vec<TokenTree>>,
    span: Span,
) -> SignatureResult<TypeData> {
    if allowed_values.is_some() {
        return fail!(span, "Options can't have restricted values");
    }
    let mutator = simple_type.mutator(&None);
    let value_type = simple_type.value();
    let sub_type = simple_type.value_type();

    Ok(TypeData {
        allowed_values: None,
        signature: format!(
            "[<any>={}...]",
            simple_type.description().to_string().to_lowercase()
        ),
        initialize: quote! { let mut #name = crate::lang::ordered_string_map::OrderedStringMap::new(); },
        mappings: quote! { (Some(name), #value_type) => #name.insert(name.to_string(), #mutator), },
        unnamed_mutate: None,
        assign: quote! { #name, },
        crush_internal_type: sub_type,
    })
}

fn vec_type_data(
    simple_type: &SimpleSignature,
    name: &Ident,
    _default: Option<TokenTree>,
    is_unnamed_target: bool,
    allowed_values: Option<Vec<TokenTree>>,
    span: Span,
) -> SignatureResult<TypeData> {
    if allowed_values.is_some() {
        return fail!(span, "Vectors can't have restricted values");
    }
    let mutator = simple_type.mutator(&None);
    let dump_all = Ident::new(simple_type.dump_list(), span.clone());
    let value_type = simple_type.value();
    let sub_type = simple_type.value_type();
    let name_literal = proc_macro2::Literal::string(&name.to_string());
    let type_name = simple_type.name();

    Ok(TypeData {
        allowed_values: None,
        crush_internal_type: quote! {crate::lang::value::ValueType::List(Box::from(#sub_type))},
        signature: format!(
            "[{}={}...]",
            name.to_string(),
            simple_type.description().to_string().to_lowercase()
        ),
        initialize: quote! { let mut #name = Vec::new(); },
        mappings: quote! {
            (Some(#name_literal), #value_type) => #name.push(#mutator),
            (Some(#name_literal), crate::lang::value::Value::List(value)) => value.#dump_all(&mut #name)?,
        },
        unnamed_mutate: if is_unnamed_target {
            Some(quote! {
                while !_unnamed.is_empty() {
                    match  _unnamed.pop_front() {
                        Some((#value_type, _location)) => #name.push(#mutator),
                    Some((_, _location)) =>
                        return crate::lang::errors::argument_error(
                            format!("Expected argument {} to be of type {}", #name_literal, #type_name),
                            _location,
                        ),
                    _ =>
                        return crate::lang::errors::argument_error_legacy(
                            format!("Missing argument {}", #name_literal)),
                    }
                }
            })
        } else {
            None
        },
        assign: quote! { #name, },
    })
}
