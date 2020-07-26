use proc_macro2;

use syn;
use proc_macro2::{TokenStream, Literal, TokenTree};
use syn::{Item, Type, PathArguments, GenericArgument, Attribute};
use quote::{quote, ToTokens, quote_spanned};
use proc_macro2::Ident;
use syn::spanned::Spanned;

struct TypeData {
    initialize: TokenStream,
    mappings: TokenStream,
    unnamed_mutate: Option<TokenStream>,
    assign: TokenStream,
    signature: String,
}

type SignatureResult<T> = Result<T, TokenStream>;

macro_rules! fail {
    ($span:expr, $msg:literal) => {
        Err(quote_spanned! {$span => compile_error!($msg);})
    }
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
            match (&path.qself, &path.path.leading_colon, path.path.segments.len()) {
                (None, None, 1) => {
                    let seg = &path.path.segments[0];
                    let s = match seg.ident.to_string().as_str() {
                        "String" => "String",
                        "Vec" => "Vec",
                        "Option" => "Option",
                        "i128" => "i128",
                        "usize" => "usize",
                        "bool" => "bool",
                        "char" => "char",
                        "f64" => "f64",
                        "Files" => "Files",
                        "ValueType" => "ValueType",
                        "PathBuf" => "PathBuf",
                        "OrderedStringMap" => "OrderedStringMap",
                        "Command" => "Command",
                        "Duration" => "Duration",
                        "Field" => "Field",
                        "Value" => "Value",
                        _ =>
                            return fail!(seg.span(), "Unrecognised type"),
                    };
                    Ok((s, extract_argument(&seg.arguments)?))
                }
                _ => fail!(ty.span(), "Invalid type")
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
        _ => false
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
                        TokenTree::Literal(l) => {
                            res.push(l)
                        }
                        TokenTree::Punct(_) => {}
                        _ => return fail!(attr.span(), "All values must be literals")
                    }
                }
            }
            _ => return fail!(attr.span(), "All values must be literals")
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
        "String" => quote!{crate::lang::value::Value::String(value)},
        "bool" => quote!{crate::lang::value::Value::Bool(value)},
        "i128" => quote!{crate::lang::value::Value::Integer(value)},
        "usize" => quote!{crate::lang::value::Value::Integer(value)},
        "ValueType" => quote!{crate::lang::value::Value::Type(value)},
        "f64" => quote!{crate::lang::value::Value::Float(value)},
        "char" => quote!{crate::lang::value::Value::String(value)},
        "Command" => quote!{crate::lang::value::Value::Command(value)},
        "Duration" => quote!{crate::lang::value::Value::Duration(value)},
        "Field" => quote!{crate::lang::value::Value::Field(value)},
        "Value" => quote!{value},
        _ => panic!("Unknown type")
    }
}

fn simple_type_to_value_description(simple_type: &str) -> &str {
    match simple_type {
        "String" => "String",
        "bool" => "Bool",
        "i128" => "Integer",
        "usize" => "Integer",
        "ValueType" => "Type",
        "f64" => "Float",
        "char" => "String",
        "Command" => "Command",
        "Duration" => "Duration",
        "Field" => "Field",
        "Value" => "Value",
        _ => panic!("Unknown type")
    }
}

fn simple_type_to_mutator(
    simple_type: &str,
    allowed_values: &Option<Ident>,
) -> TokenStream {
    match allowed_values {
        None => {
            match simple_type {
                "char" => quote! { if value.len() == 1 { value.chars().next().unwrap()} else {return crate::lang::errors::argument_error("Argument must be exactly one character")}},
                "usize" => quote! { crate::lang::errors::to_crush_error(usize::try_from(value))?},
                _ => quote! {value},
            }
        }
        Some(allowed) => {
            match simple_type {
                "char" => quote! {
                    if value.len() == 1 {
                        let c = value.chars().next().unwrap();
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
                    if #allowed.contains(&value.as_str()) {
                        value
                    } else {
                        return crate::lang::errors::argument_error(format!("Only the following values are allowed: {:?}", #allowed).as_str())
                    }
                },
                _ => quote! {
                    if #allowed.contains(&value) {
                        value
                    } else {
                        return crate::lang::errors::argument_error(format!("Only the following values are allowed: {:?}", #allowed).as_str())
                    }
                },
            }
        }
    }
}

fn simple_type_dump_list(simple_type: &str) -> &str {
    match simple_type {
        "String" => "dump_string",
        "bool" => "dump_bool",
        "i128" => "dump_integer",
        "ValueType" => "dump_type",
        "f64" => "dump_float",
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

    let allowed_values_name =
        allowed_values.as_ref().map(|_| Ident::new(&format!("{}_allowed_values", name.to_string()), ty.span()));

    let (type_name, args) = extract_type(ty)?;
    match type_name {
        "i128" | "bool" | "String" | "char" | "ValueType" | "f64" | "Command" | "Duration" | "Field" | "Value" | "usize" => {
            if !args.is_empty() {
                fail!(ty.span(), "This type can't be paramterizised")
            } else {
                let native_type = Ident::new(type_name, ty.span());
                let mutator = simple_type_to_mutator(type_name, &allowed_values_name);
                let value_type = simple_type_to_value(type_name);
                Ok(TypeData {
                    signature:
                    if default.is_none() {
                        format!("{}={}", name.to_string(), simple_type_to_value_description(type_name).to_string().to_lowercase())
                    } else {
                        format!("[{}={}]", name.to_string(), simple_type_to_value_description(type_name).to_string().to_lowercase())
                    }
                    ,
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
                    unnamed_mutate:
                    match default {
                        None => {
                            Some(quote! {
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
                            })
                        }
                        Some(def) => {
                            Some(quote! {
if #name.is_none() {
    match _unnamed.pop_front() {
        Some(#value_type) => #name = Some(#mutator),
        None => #name = Some(#native_type::from(#def)),
        _ => return crate::lang::errors::argument_error(format!("Expected argument {} to be of type {}", #name_literal, #type_name).as_str()),
        }
}
                            })
                        }
                    },
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
                    signature: format!("[{}=(file|glob|regex|list|table|table_stream)...]", name.to_string()),
                    initialize: quote! { let mut #name = crate::lang::files::Files::new(); },
                    mappings: quote! { (Some(#name_literal), value) => #name.expand(value, printer)?, },
                    unnamed_mutate: if is_unnamed_target {
                        Some(quote! {
                            while !_unnamed.is_empty() {
                                #name.expand(_unnamed.pop_front().unwrap(), printer)?;
                            }
                        })
                    } else { None },
                    assign: quote! { #name, },
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

                Ok(TypeData {
                    signature: format!("[{}={}...]", name.to_string(), simple_type_to_value_description(args[0]).to_string().to_lowercase()),
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
                    } else { None },
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

                Ok(TypeData {
                    signature: format!("[<any>={}...]", simple_type_to_value_description(args[0]).to_string().to_lowercase()),
                    initialize: quote! { let mut #name = crate::lang::ordered_string_map::OrderedStringMap::new(); },
                    mappings: quote! { (Some(name), #value_type) => #name.insert(name.to_string(), #mutator), },
                    unnamed_mutate: None,
                    assign: quote! { #name, },
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
                    signature: format!("[{}={}]", name.to_string(), simple_type_to_value_description(args[0]).to_string().to_lowercase()),
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
                            }
                    ),
                    assign: quote! { #name, },
                })
            }
        }

        _ => fail!(ty.span(), "Unsupported type"),
    }
}

struct Metadata {
    name: String,
    can_block: bool,
    short_description: Option<String>,
    long_description: Vec<String>,
    example: Option<String>,
    output: Option<TokenStream>,
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

    let location = metadata.span().clone();
    let metadata_iter = metadata.into_iter().collect::<Vec<_>>();
    let v = metadata_iter.split(|e| {
        match e {
            TokenTree::Punct(p) => {
                p.as_char() == ','
            }
            _ => false,
        }
    }).collect::<Vec<_>>();
    if v.len() == 0 {
        return fail!(location, "No name specified");
    }
    let name = match v[0].clone() {
        [TokenTree::Ident(i)] => {
            i.to_string()
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
                            ("can_block", '=') => can_block = match l.to_string().as_str() {
                                "true" => true,
                                "false" => false,
                                _ => return fail!(l.span(), "Expected a boolean value"),
                            },
                            _ => return fail!(l.span(), "Unknown argument"),
                        }
                    }
                    _ => return fail!(meta[0].span(), "Invalid parameter format"),
                }
            }
        }
    }

    Ok(Metadata {
        name,
        can_block,
        short_description,
        long_description,
        example,
        output,
    })
}

fn signature_real(metadata: TokenStream, input: TokenStream) -> SignatureResult<TokenStream> {
    let metadata_location = metadata.span();
    let metadata = parse_metadata(metadata)?;

    let description = Literal::string(metadata.short_description.unwrap_or_else(|| "Missing description".to_string()).as_str());

    let command_invocation = Ident::new(&metadata.name, metadata_location);
    let command_name = Literal::string(&metadata.name);
    let can_block = Ident::new(if metadata.can_block { "true" } else { "false" }, metadata_location);

    let root: syn::Item = syn::parse2(input).expect("Invalid syntax tree");

    let mut long_description = metadata.long_description;
    let mut signature = vec![metadata.name.to_string()];
    let output = metadata.output
        .map(|o| quote!{#o} )
        .unwrap_or(quote!{crate::lang::command::OutputType::Unknown});

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
                let type_data = type_to_value(&field.ty, name, default_value.clone(), is_unnamed_target, allowed_values)?;

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
                        long_description.push("This command accepts the following arguments:".to_string());
                        had_field_description = true;
                    }
                    long_description.push(format!("* {}{}, {}", name.to_string(), default_help, description));
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

impl crate::lang::argument::ArgumentHandler for #struct_name {
    fn declare(env: &mut crate::lang::scope::ScopeLoader) -> crate::lang::errors::CrushResult <()> {
        env.declare_command(
            #command_name, #command_invocation, #can_block,
            #signature_literal,
            #description,
            #long_description,
            #output)
    }

    fn declare_method(env: &mut ordered_map::OrderedMap<std::string::String, crate::lang::command::Command>, path: &Vec<&str>) -> crate::lang::errors::CrushResult <()> {
        let mut full = path.clone();
        full.push(#command_name);
        env.insert(#command_name.to_string(),
                    crate::lang::command::CrushCommand::command(
                        #command_invocation, #can_block, full.iter().map(|e| e.to_string()).collect(),
                        #signature_literal, #description, #long_description, #output));
        Ok(())
    }

    fn parse(arguments: Vec<crate::lang::argument::Argument>, printer: &crate::lang::printer::Printer) -> crate::lang::errors::CrushResult < # struct_name > {
        use std::convert::TryFrom;
        # values
        let mut _unnamed = std::collections::VecDeque::new();

        for arg in arguments {
            match (arg.argument_type.as_deref(), arg.value) {
                #named_matchers
                #named_fallback
                (None, value) => _unnamed.push_back(value),
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
        _ => { fail!(root.span(), "Expected a struct") }
    }
}

#[proc_macro_attribute]
pub fn signature(metadata: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match signature_real(TokenStream::from(metadata), TokenStream::from(input)) {
        Ok(res) | Err(res) => {
            proc_macro::TokenStream::from(res)
        }
    }
}

