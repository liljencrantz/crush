use proc_macro2;
use proc_macro2::Ident;
use proc_macro2::{Literal, TokenStream, TokenTree};
use quote::{quote, quote_spanned, ToTokens};
use syn;
use syn::spanned::Spanned;
use syn::{Attribute, GenericArgument, Item, PathArguments, Type};

struct TypeData {
    initialize: TokenStream,
    mappings: TokenStream,
    unnamed_mutate: Option<TokenStream>,
    assign: TokenStream,
    crush_internal_type: TokenStream,
    signature: String,
}

type SignatureResult<T> = Result<T, TokenStream>;

macro_rules! fail {
    ($span:expr, $msg:literal) => {
        Err(quote_spanned! {$span => compile_error!($msg);})
    };
}

fn extract_argument(path: &PathArguments) -> SignatureResult<Vec<&'static str>> {
    match path {
        PathArguments::None => Ok(Vec::new()),
        PathArguments::AngleBracketed(a) => {
            let mut res = Vec::new();
            for g in &a.args {
                match g {
                    GenericArgument::Type(t) => {
                        res.push(extract_type(t)?.0);
                    }
                    _ => return fail!(path.span(), "Expected a type"),
                }
            }
            Ok(res)
        }
        PathArguments::Parenthesized(_) => Ok(Vec::new()),
    }
}

fn extract_type(ty: &Type) -> SignatureResult<(&'static str, Vec<&'static str>)> {
    match ty {
        Type::Path(path) => {
            match (
                &path.qself,
                &path.path.leading_colon,
                path.path.segments.len(),
            ) {
                (None, None, 1) => {
                    let seg = &path.path.segments[0];
                    let s = match seg.ident.to_string().as_str() {
                        "String" => "String",
                        "Vec" => "Vec",
                        "Option" => "Option",
                        "i128" => "i128",
                        "i64" => "i64",
                        "u64" => "u64",
                        "usize" => "usize",
                        "bool" => "bool",
                        "char" => "char",
                        "f64" => "f64",
                        "Files" => "Files",
                        "Patterns" => "Patterns",
                        "ValueType" => "ValueType",
                        "PathBuf" => "PathBuf",
                        "OrderedStringMap" => "OrderedStringMap",
                        "Command" => "Command",
                        "Duration" => "Duration",
                        "Struct" => "Struct",
                        "Field" => "Field",
                        "Value" => "Value",
                        "Stream" => "Stream",
                        "Number" => "Number",
                        _ => return fail!(seg.span(), "Unrecognised type"),
                    };
                    Ok((s, extract_argument(&seg.arguments)?))
                }
                _ => fail!(ty.span(), "Invalid type"),
            }
        }
        _ => fail!(ty.span(), "Invalid type, expected a Path segment"),
    }
}

fn call_is_named(attr: &Attribute, name: &str) -> bool {
    let path = &attr.path;
    match (&path.leading_colon, path.segments.len()) {
        (None, 1) => {
            let seg = &path.segments[0];
            seg.ident.to_string().as_str() == name
        }
        _ => false,
    }
}

fn call_is_default(attr: &Attribute) -> bool {
    call_is_named(attr, "default")
}

fn call_values(attr: &Attribute) -> SignatureResult<Vec<TokenTree>> {
    let mut res = Vec::new();
    for tree in attr.tokens.clone().into_iter() {
        res.push(tree);
    }
    Ok(res)
}

fn call_literals(attr: &Attribute) -> SignatureResult<Vec<Literal>> {
    let mut res = Vec::new();
    for tree in attr.tokens.clone().into_iter() {
        match tree {
            TokenTree::Group(g) => {
                for item in g.stream().into_iter() {
                    match item {
                        TokenTree::Literal(l) => res.push(l),
                        TokenTree::Punct(_) => {}
                        _ => return fail!(attr.span(), "All values must be literals"),
                    }
                }
            }
            _ => return fail!(attr.span(), "All values must be literals"),
        }
    }
    Ok(res)
}

fn call_literal(attr: &Attribute) -> SignatureResult<Literal> {
    let mut res = call_literals(attr)?;
    if res.len() == 1 {
        Ok(res.remove(0))
    } else {
        fail!(attr.span(), "Expected one description argument")
    }
}

fn call_value(attr: &Attribute) -> SignatureResult<TokenTree> {
    let values = call_values(attr)?;
    if values.len() == 1 {
        Ok(values[0].clone())
    } else {
        fail!(attr.span(), "Expected exactly one literal")
    }
}

fn simple_type_to_value(simple_type: &str) -> TokenStream {
    match simple_type {
        "String" => quote! {crate::lang::value::Value::String(_value)},
        "bool" => quote! {crate::lang::value::Value::Bool(_value)},
        "i128" => quote! {crate::lang::value::Value::Integer(_value)},
        "usize" => quote! {crate::lang::value::Value::Integer(_value)},
        "u64" => quote! {crate::lang::value::Value::Integer(_value)},
        "i64" => quote! {crate::lang::value::Value::Integer(_value)},
        "ValueType" => quote! {crate::lang::value::Value::Type(_value)},
        "f64" => quote! {crate::lang::value::Value::Float(_value)},
        "char" => quote! {crate::lang::value::Value::String(_value)},
        "Command" => quote! {crate::lang::value::Value::Command(_value)},
        "Duration" => quote! {crate::lang::value::Value::Duration(_value)},
        "Field" => quote! {crate::lang::value::Value::Field(_value)},
        "Struct" => quote! {crate::lang::value::Value::Struct(_value)},
        "Stream" => quote! {_value},
        "Value" => quote! {_value},
        _ => panic!("Unknown type"),
    }
}

fn simple_type_to_value_type(simple_type: &str) -> TokenStream {
    match simple_type {
        "String" => quote! {crate::lang::value::ValueType::String},
        "bool" => quote! {crate::lang::value::ValueType::Bool},
        "i128" => quote! {crate::lang::value::ValueType::Integer},
        "usize" => quote! {crate::lang::value::ValueType::Integer},
        "u64" => quote! {crate::lang::value::ValueType::Integer},
        "i64" => quote! {crate::lang::value::ValueType::Integer},
        "ValueType" => quote! {crate::lang::value::ValueType::Type},
        "f64" => quote! {crate::lang::value::ValueType::Float},
        "char" => quote! {crate::lang::value::ValueType::String},
        "Command" => quote! {crate::lang::value::ValueType::Command},
        "Duration" => quote! {crate::lang::value::ValueType::Duration},
        "Field" => quote! {crate::lang::value::ValueType::Field},
        "Struct" => quote! {crate::lang::value::ValueType::Struct},
        _ => quote! {crate::lang::value::ValueType::Any},
    }
}

fn simple_type_to_value_description(simple_type: &str) -> &str {
    match simple_type {
        "String" => "string",
        "bool" => "bool",
        "i128" => "integer",
        "usize" => "integer",
        "u64" => "integer",
        "i64" => "integer",
        "ValueType" => "type",
        "f64" => "float",
        "char" => "string",
        "Command" => "command",
        "Duration" => "duration",
        "Field" => "field",
        "Value" => "any value",
        "Stream" => "stream",
        "Struct" => "Struct",
        _ => panic!("Unknown type"),
    }
}

fn simple_type_to_mutator(simple_type: &str, allowed_values: &Option<Ident>) -> TokenStream {
    match allowed_values {
        None => match simple_type {
            "char" => {
                quote! { if _value.len() == 1 { _value.chars().next().unwrap()} else {return crate::lang::errors::argument_error("Argument must be exactly one character")}}
            }
            "usize" => quote! { crate::lang::errors::to_crush_error(usize::try_from(_value))?},
            "u64" => quote! { crate::lang::errors::to_crush_error(u64::try_from(_value))?},
            "i64" => quote! { crate::lang::errors::to_crush_error(i64::try_from(_value))?},
            "Stream" => {
                quote! { crate::lang::errors::mandate(_value.stream(), "Expected a type that can be streamed")? }
            }
            _ => quote! {_value},
        },
        Some(allowed) => match simple_type {
            "char" => quote! {
                if _value.len() == 1 {
                    let c = _value.chars().next().unwrap();
                    if #allowed.contains(&c) {
                        c
                    } else {
                        return crate::lang::errors::argument_error(format!("Only the following values are allowed: {:?}", #allowed).as_str())
                    }
                } else {
                    return crate::lang::errors::argument_error("Argument must be exactly one character")
                }
            },
            "String" => quote! {
                if #allowed.contains(&_value.as_str()) {
                    _value
                } else {
                    return crate::lang::errors::argument_error(format!("Only the following values are allowed: {:?}", #allowed).as_str())
                }
            },
            _ => quote! {
                if #allowed.contains(&_value) {
                    _value
                } else {
                    return crate::lang::errors::argument_error(format!("Only the following values are allowed: {:?}", #allowed).as_str())
                }
            },
        },
    }
}

fn simple_type_dump_list(simple_type: &str) -> &str {
    match simple_type {
        "String" => "dump_string",
        "bool" => "dump_bool",
        "i128" => "dump_integer",
        "ValueType" => "dump_type",
        "f64" => "dump_float",
        "Value" => "dump_value",
        "Field" => "dump_field",
        _ => panic!("Unknown type"),
    }
}

fn type_to_value(
    ty: &Type,
    name: &Ident,
    default: Option<TokenTree>,
    is_unnamed_target: bool,
    allowed_values: Option<Vec<Literal>>,
) -> SignatureResult<TypeData> {
    let name_literal = proc_macro2::Literal::string(&name.to_string());

    let allowed_values_name = allowed_values
        .as_ref()
        .map(|_| Ident::new(&format!("{}_allowed_values", name.to_string()), ty.span()));

    let (type_name, args) = extract_type(ty)?;
    match type_name {
        "i128" | "bool" | "String" | "char" | "ValueType" | "f64" | "Command" | "Duration"
        | "Field" | "Value" | "usize" | "i64" | "u64" | "Stream" | "Struct" => {
            if !args.is_empty() {
                fail!(ty.span(), "This type can't be paramterizised")
            } else {
                let native_type = Ident::new(type_name, ty.span());
                let mutator = simple_type_to_mutator(type_name, &allowed_values_name);
                let value_type = simple_type_to_value(type_name);
                Ok(TypeData {
                    crush_internal_type: simple_type_to_value_type(type_name),
                    signature: if default.is_none() {
                        format!(
                            "{}={}",
                            name.to_string(),
                            simple_type_to_value_description(type_name)
                                .to_string()
                                .to_lowercase()
                        )
                    } else {
                        format!(
                            "[{}={}]",
                            name.to_string(),
                            simple_type_to_value_description(type_name)
                                .to_string()
                                .to_lowercase()
                        )
                    },
                    initialize: match allowed_values {
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
                    mappings: quote! {(Some(#name_literal), #value_type) => #name = Some(#mutator),},
                    unnamed_mutate: match default {
                        None => Some(quote! {
                        if #name.is_none() {
                            match _unnamed.pop_front() {
                                Some(#value_type) => #name = Some(#mutator),
                                Some(value) =>
                                    return crate::lang::errors::argument_error(format!(
                                        "Expected argument \"{}\" to be of type {}, was of type {}",
                                        #name_literal,
                                        #type_name,
                                        value.value_type().to_string()).as_str()),
                                _ =>
                                    return crate::lang::errors::argument_error(format!(
                                        "No value provided for argument \"{}\"",
                                        #name_literal).as_str()),
                            }
                        }
                                                    }),
                        Some(def) => Some(quote! {
                        if #name.is_none() {
                            match _unnamed.pop_front() {
                                Some(#value_type) => #name = Some(#mutator),
                                None => #name = Some(#native_type::from(#def)),
                                _ => return crate::lang::errors::argument_error(format!("Expected argument {} to be of type {}", #name_literal, #type_name).as_str()),
                                }
                        }
                                                    }),
                    },
                    assign: quote! {
                    #name: crate::lang::errors::mandate(
                        #name,
                        format!("Missing value for parameter {}", #name_literal).as_str())?,
                    },
                })
            }
        }

        "Number" => {
            if !args.is_empty() {
                fail!(ty.span(), "This type can't be paramterizised")
            } else {
                Ok(TypeData {
                    crush_internal_type: quote!{crate::lang::value::ValueType::either(vec![
                        crate::lang::value::ValueType::Integer,
                        crate::lang::value::ValueType::Float,
                    ])},
                    signature: format!(
                        "{}=(float|integer)",
                        name.to_string()
                    ),
                    initialize: quote! { let mut #name = None; },
                    mappings: quote! {
                        (Some(#name_literal), crate::lang::value::Value::Float(_value)) => #name = Some(Number::Float(_value)),
                        (Some(#name_literal), crate::lang::value::Value::Integer(_value)) => #name = Some(Number::Integer(_value)),
                    },
                    unnamed_mutate: Some(quote! {
                        if #name.is_none() {
                            match _unnamed.pop_front() {
                                Some(crate::lang::value::Value::Float(_value)) => #name = Some(Number::Float(_value)),
                                Some(crate::lang::value::Value::Integer(_value)) => #name = Some(Number::Integer(_value)),
                                Some(value) =>
                                    return crate::lang::errors::argument_error(format!(
                                        "Expected argument \"{}\" to be a number, was of type {}",
                                        #name_literal,
                                        value.value_type().to_string()).as_str()),
                                _ =>
                                    return crate::lang::errors::argument_error(format!(
                                        "No value provided for argument \"{}\"",
                                        #name_literal).as_str()),
                             }
                        }
                    }),
                    assign: quote! {
                    #name: crate::lang::errors::mandate(
                        #name,
                        format!("Missing value for parameter {}", #name_literal).as_str())?,
                    },
                })
            }
        }

        "Files" => {
            if !args.is_empty() {
                fail!(ty.span(), "This type can't be paramterizised")
            } else {
                Ok(TypeData {
                    signature: format!(
                        "[{}=(file|glob|regex|list|table|table_stream)...]",
                        name.to_string()
                    ),
                    initialize: quote! { let mut #name = crate::lang::files::Files::new(); },
                    mappings: quote! { (Some(#name_literal), value) => #name.expand(value, _printer)?, },
                    unnamed_mutate: if is_unnamed_target {
                        Some(quote! {
                            while !_unnamed.is_empty() {
                                #name.expand(_unnamed.pop_front().unwrap(), _printer)?;
                            }
                        })
                    } else {
                        None
                    },
                    assign: quote! { #name, },
                    crush_internal_type: quote!{crate::lang::value::ValueType::Any},
                })
            }
        }

        "Patterns" => {
            if !args.is_empty() {
                fail!(ty.span(), "This type can't be paramterizised")
            } else {
                Ok(TypeData {
                    signature: format!("[{}=(string|glob|regex)...]", name.to_string()),
                    initialize: quote! { let mut #name = crate::lang::patterns::Patterns::new(); },
                    mappings: quote! {
                        (Some(#name_literal), crate::lang::value::Value::Glob(value)) => #name.expand_glob(value),
                        (Some(#name_literal), crate::lang::value::Value::String(value)) => #name.expand_string(value),
                        (Some(#name_literal), crate::lang::value::Value::Regex(pattern, value)) => #name.expand_regex(pattern, value),
                    },
                    unnamed_mutate: if is_unnamed_target {
                        Some(quote! {
                            while !_unnamed.is_empty() {
                                match _unnamed.pop_front().unwrap() {
                        crate::lang::value::Value::Glob(value) => #name.expand_glob(value),
                        crate::lang::value::Value::String(value) => #name.expand_string(value),
                        crate::lang::value::Value::Regex(pattern, value) => #name.expand_regex(pattern, value),
                                }
                            }
                        })
                    } else {
                        None
                    },
                    assign: quote! { #name, },
                    crush_internal_type: quote!{crate::lang::value::ValueType::either(vec![
                        crate::lang::value::ValueType::String,
                        crate::lang::value::ValueType::Glob,
                        crate::lang::value::ValueType::Regex,
                    ])},
                })
            }
        }

        "Vec" => {
            if allowed_values.is_some() {
                return fail!(ty.span(), "Vectors can't have restricted values");
            }
            if args.len() != 1 {
                fail!(ty.span(), "Vec needs exactly one parameter")
            } else {
                let mutator = simple_type_to_mutator(args[0], &None);
                let dump_all = Ident::new(simple_type_dump_list(args[0]), ty.span().clone());
                let value_type = simple_type_to_value(args[0]);
                let sub_type = simple_type_to_value_type(args[0]);

                Ok(TypeData {
                    crush_internal_type: quote!{crate::lang::value::ValueType::List(Box::from(#sub_type))},
                    signature: format!(
                        "[{}={}...]",
                        name.to_string(),
                        simple_type_to_value_description(args[0])
                            .to_string()
                            .to_lowercase()
                    ),
                    initialize: quote! { let mut #name = Vec::new(); },
                    mappings: quote! {
                        (Some(#name_literal), #value_type) => #name.push(#mutator),
                        (Some(#name_literal), crate::lang::value::Value::List(value)) => value.#dump_all(&mut #name)?,
                    },
                    unnamed_mutate: if is_unnamed_target {
                        Some(quote! {
                            while !_unnamed.is_empty() {
                                if let Some(#value_type) = _unnamed.pop_front() {
                                    #name.push(#mutator);
                                } else {
                                    return crate::lang::errors::argument_error(format!("Expected argument {} to be of type {}", #name_literal, #type_name).as_str());
                                }
                            }
                        })
                    } else {
                        None
                    },
                    assign: quote! { #name, },
                })
            }
        }

        "OrderedStringMap" => {
            if allowed_values.is_some() {
                return fail!(ty.span(), "Options can't have restricted values");
            }
            if args.len() != 1 {
                fail!(ty.span(), "OrderedStringMap needs exactly one parameter")
            } else {
                let mutator = simple_type_to_mutator(args[0], &None);
                let value_type = simple_type_to_value(args[0]);
                let sub_type = simple_type_to_value_type(args[0]);

                Ok(TypeData {
                    signature: format!(
                        "[<any>={}...]",
                        simple_type_to_value_description(args[0])
                            .to_string()
                            .to_lowercase()
                    ),
                    initialize: quote! { let mut #name = crate::lang::ordered_string_map::OrderedStringMap::new(); },
                    mappings: quote! { (Some(name), #value_type) => #name.insert(name.to_string(), #mutator), },
                    unnamed_mutate: None,
                    assign: quote! { #name, },
                    crush_internal_type: sub_type,
                })
            }
        }

        "Option" => {
            if args.len() != 1 {
                fail!(ty.span(), "Option needs exactly on parameter")
            } else {
                let sub_type = Literal::string(args[0]);
                let mutator = simple_type_to_mutator(args[0], &None);
                let value_type = simple_type_to_value(args[0]);

                Ok(TypeData {
                    signature: format!(
                        "[{}={}]",
                        name.to_string(),
                        simple_type_to_value_description(args[0])
                            .to_string()
                            .to_lowercase()
                    ),
                    initialize: quote! { let mut #name = None; },
                    mappings: quote! { (Some(#name_literal), #value_type) => #name = Some(#mutator), },
                    unnamed_mutate: Some(quote_spanned! { ty.span() =>
                    if #name.is_none() {
                        match _unnamed.pop_front() {
                            None => {}
                            Some(#value_type) => #name = Some(#mutator),
                            Some(_) => return crate::lang::errors::argument_error(format!("Expected argument {} to be of type {}", #name_literal, #sub_type).as_str()),
                        }
                    }
                    }),
                    assign: quote! { #name, },
                    crush_internal_type: simple_type_to_value_type(args[0]),
                })
            }
        }

        _ => fail!(ty.span(), "Unsupported type"),
    }
}

struct Metadata {
    identifier: Ident,
    name: String,
    can_block: bool,
    short_description: Option<String>,
    long_description: Vec<String>,
    example: Option<String>,
    output: Option<TokenStream>,
    #[allow(unused)]
    condition: bool,
}

fn unescape(s: &str) -> String {
    let mut res = "".to_string();
    let mut was_backslash = false;
    for c in s[1..s.len() - 1].chars() {
        if was_backslash {
            match c {
                'n' => res += "\n",
                'r' => res += "\r",
                't' => res += "\t",
                _ => res += &c.to_string(),
            }
            was_backslash = false;
        } else if c == '\\' {
            was_backslash = true;
        } else {
            res += &c.to_string();
        }
    }
    res
}

fn parse_metadata(metadata: TokenStream) -> SignatureResult<Metadata> {
    let mut can_block = true;
    let mut example = None;
    let mut short_description = None;
    let mut long_description = Vec::new();
    let mut output: Option<TokenStream> = None;
    let mut condition = false;

    let location = metadata.span().clone();
    let metadata_iter = metadata.into_iter().collect::<Vec<_>>();
    let v = metadata_iter
        .split(|e| match e {
            TokenTree::Punct(p) => p.as_char() == ',',
            _ => false,
        })
        .collect::<Vec<_>>();
    if v.len() == 0 {
        return fail!(location, "No name specified");
    }
    let (name, identifier) = match v[0].clone() {
        [TokenTree::Ident(i)] => {
            let as_str = i.to_string();
            if as_str.starts_with("r#") {
                let mut ch = as_str.chars();
                ch.next();
                ch.next();
                (ch.as_str().to_string(), i.clone())
            } else {
                (as_str, i.clone())
            }
        }
        _ => {
            return fail!(location, "Invalid name");
        }
    };

    for meta in &v[1..] {
        if meta.len() == 0 {
            continue;
        }
        if let TokenTree::Ident(name) = &meta[0] {
            if name.to_string().as_str() == "output" && meta.len() > 2 {
                let mut tmp = TokenStream::new();
                for s in &meta[2..] {
                    tmp.extend(s.into_token_stream());
                }
                output = Some(tmp);
            } else {
                if meta.len() != 3 {
                    return fail!(meta[0].span(), "Invalid parameter format");
                }
                match (&meta[1], &meta[2]) {
                    (TokenTree::Punct(p), TokenTree::Literal(l)) => {
                        let unescaped = unescape(&l.to_string());
                        match (name.to_string().as_str(), p.as_char()) {
                            ("short", '=') => short_description = Some(unescaped),
                            ("long", '=') => long_description.push(unescaped),
                            ("example", '=') => example = Some(unescaped),
                            _ => return fail!(l.span(), "Unknown argument"),
                        }
                    }
                    (TokenTree::Punct(p), TokenTree::Ident(l)) => {
                        match (name.to_string().as_str(), p.as_char()) {
                            ("can_block", '=') => {
                                can_block = match l.to_string().as_str() {
                                    "true" => true,
                                    "false" => false,
                                    _ => return fail!(l.span(), "Expected a boolean value"),
                                }
                            }
                            ("condition", '=') => {
                                condition = match l.to_string().as_str() {
                                    "true" => true,
                                    "false" => false,
                                    _ => return fail!(l.span(), "Expected a boolean value"),
                                }
                            }
                            _ => return fail!(l.span(), "Unknown argument"),
                        }
                    }
                    _ => return fail!(meta[0].span(), "Invalid parameter format"),
                }
            }
        }
    }

    Ok(Metadata {
        identifier,
        name,
        can_block,
        short_description,
        long_description,
        example,
        output,
        condition,
    })
}

fn signature_real(metadata: TokenStream, input: TokenStream) -> SignatureResult<TokenStream> {
    let metadata_location = metadata.span();
    let metadata = parse_metadata(metadata)?;

    let description = Literal::string(
        metadata
            .short_description
            .unwrap_or_else(|| "Missing description".to_string())
            .as_str(),
    );

    let command_invocation = metadata.identifier;
    let command_name = Literal::string(&metadata.name);
    let can_block = Ident::new(
        if metadata.can_block { "true" } else { "false" },
        metadata_location,
    );

    let root: syn::Item = syn::parse2(input).expect("Invalid syntax tree");

    let mut long_description = metadata.long_description;
    let mut signature = vec![metadata.name.to_string()];
    let output = metadata
        .output
        .map(|o| quote! {#o})
        .unwrap_or(quote! {crate::lang::command::OutputType::Unknown});

    match root {
        Item::Struct(mut s) => {
            let mut named_matchers = proc_macro2::TokenStream::new();
            let mut values = proc_macro2::TokenStream::new();
            let mut unnamed_mutations = proc_macro2::TokenStream::new();
            let mut assignments = proc_macro2::TokenStream::new();
            let mut named_fallback = proc_macro2::TokenStream::new();
            let mut had_unnamed_target = false;
            let struct_name = s.ident.clone();
            let mut had_field_description = false;

            let mut argument_desciptions = quote!{};

            for field in &mut s.fields {
                let mut default_value = None;
                let mut is_unnamed_target = false;
                let mut is_named_target = false;
                let mut allowed_values = None;
                let mut description = None;
                if !field.attrs.is_empty() {
                    for attr in &field.attrs {
                        if call_is_default(attr) {
                            default_value = Some(call_value(attr)?)
                        } else if call_is_named(attr, "unnamed") {
                            is_unnamed_target = true;
                        } else if call_is_named(attr, "named") {
                            is_named_target = true;
                        } else if call_is_named(attr, "values") {
                            allowed_values = Some(call_literals(attr)?);
                        } else if call_is_named(attr, "description") {
                            description = Some(unescape(&(call_literal(attr)?.to_string())));
                        }
                    }
                }
                field.attrs = Vec::new();
                let name = &field.ident.clone().unwrap();
                let name_string = Literal::string(&name.to_string());
                let type_data = type_to_value(
                    &field.ty,
                    name,
                    default_value.clone(),
                    is_unnamed_target,
                    allowed_values,
                )?;

                signature.push(type_data.signature);

                let initialize = type_data.initialize;
                let mappings = type_data.mappings;
                values.extend(initialize);

                if is_named_target {
                    named_fallback.extend(mappings)
                } else {
                    named_matchers.extend(mappings);
                }

                let default_help = if let Some(d) = &default_value {
                    format!(" {}", d.to_string())
                } else {
                    "".to_string()
                };
                if let Some(description) = description {
                    if !had_field_description {
                        long_description
                            .push("This command accepts the following arguments:".to_string());
                        had_field_description = true;
                    }
                    long_description.push(format!(
                        "* {}{}, {}",
                        name.to_string(),
                        default_help,
                        description
                    ));
                }

                if !had_unnamed_target || default_value.is_some() {
                    if let Some(mutate) = type_data.unnamed_mutate {
                        unnamed_mutations.extend(quote! {
                            #mutate
                        });
                    }
                }

                assignments.extend(type_data.assign);
                had_unnamed_target |= is_unnamed_target;
                let crush_internal_type = type_data.crush_internal_type;

                argument_desciptions = quote!{
                    #argument_desciptions
                    crate::lang::command::ArgumentDescription {
                        name: #name_string.to_string(),
                        value_type: #crush_internal_type,
                        allowed: None,
                        description: None,
                        complete: None,
                        named: false,
                        unnamed: false,
                    },
                };
            }

            if let Some(example) = metadata.example {
                long_description.push("Example".to_string());
                long_description.push(example);
            }

            let signature_literal = Literal::string(&signature.join(" "));

            let long_description = if !long_description.is_empty() {
                let mut s = "    ".to_string();
                s.push_str(&long_description.join("\n\n    "));
                let text = Literal::string(&s);
                quote! {Some(#text) }
            } else {
                quote! {None}
            };

            let handler = quote! {

            #[allow(unused_parens)] // TODO: don't emit unnecessary parenthesis in the first place
            impl #struct_name {
                pub fn declare(env: &mut crate::lang::data::scope::ScopeLoader) -> crate::lang::errors::CrushResult <()> {
                    env.declare_command(
                        #command_name,
                        #command_invocation,
                        #can_block,
                        #signature_literal,
                        #description,
                        #long_description,
                        #output,
                        vec![
                            #argument_desciptions
                        ],
                    )
                }

                pub fn declare_method(env: &mut ordered_map::OrderedMap<std::string::String, crate::lang::command::Command>, path: &Vec<&str>) {
                    let mut full = path.clone();
                    full.push(#command_name);
                    env.insert(#command_name.to_string(),
                        crate::lang::command::CrushCommand::command(
                            #command_invocation,
                            #can_block,
                            full.iter().map(|e| e.to_string()).collect(),
                            #signature_literal,
                            #description,
                            #long_description,
                            #output,
                            vec![
                                #argument_desciptions
                            ],
                        )
                    );
                }

                pub fn parse(_arguments: Vec<crate::lang::argument::Argument>, _printer: &crate::lang::printer::Printer) -> crate::lang::errors::CrushResult < # struct_name > {
                    use std::convert::TryFrom;
                    # values
                    let mut _unnamed = std::collections::VecDeque::new();

                    for arg in _arguments {
                        match (arg.argument_type.as_deref(), arg.value) {
                            #named_matchers
                            #named_fallback
                            (None, _value) => _unnamed.push_back(_value),
                            _ => return crate::lang::errors::argument_error("Invalid parameter"),
                        }
                    }

                    #unnamed_mutations

                    Ok( #struct_name { #assignments })
                }
            }
            };

            let mut output = s.to_token_stream();
            output.extend(handler.into_token_stream());
            //println!("ABCABC {}", output.to_string());
            Ok(output)
        }
        _ => fail!(root.span(), "Expected a struct"),
    }
}

#[proc_macro_attribute]
pub fn signature(
    metadata: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match signature_real(TokenStream::from(metadata), TokenStream::from(input)) {
        Ok(res) | Err(res) => proc_macro::TokenStream::from(res),
    }
}
