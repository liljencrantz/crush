/**
This macro is used on command signature structs. It outputs methods that let you
parse and declare a command based on its signature.
 */

use proc_macro2;
use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, Span};
use proc_macro2::{Literal, TokenStream, TokenTree};
use quote::{quote, quote_spanned, ToTokens};
use syn::{Attribute, Item};
use simple_signature::SimpleSignature;
use crate::signature::Signature;
use syn::spanned::Spanned;

mod simple_signature;
mod signature;

macro_rules! fail {
    ($span:expr, $msg:literal) => {
        Err(quote_spanned! {$span => compile_error!($msg);})
    };
}

type SignatureResult<T> = Result<T, TokenStream>;

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
    path: Vec<String>,
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

fn parse_full_name(location: Span, name_tree: &[TokenTree]) -> SignatureResult<(String, Ident, Vec<String>)> {
    let mut res = vec![];
    for el in name_tree.into_iter() {
        match el {
            TokenTree::Ident(l) =>
                res.push(l),
            TokenTree::Punct(p) => {
                if p.as_char() != '.' {
                    return fail!(el.span(), "Unbexpected punctuation");
                }
            }
            TokenTree::Group(_) | TokenTree::Literal(_) => {
                return fail!(el.span(), "Expected identifier");
            }
        }
    }

    if res.len() < 1 {
        return fail!(location.span(), "Expected identifier");
    }

    let i = res.pop().unwrap();
    let as_str = i.to_string();
    let mut ch = as_str.chars();
    if as_str.starts_with("r#") {
        ch.next();
        ch.next();
    }
    return Ok((ch.as_str().to_string(), i.clone(), res.iter().map(|id| id.to_string()).collect()));
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

    let (name, identifier, path) = parse_full_name(location, v[0])?;

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
        path,
    })
}

fn generate_signature(path: &[String], signature: Vec<String>) -> String {
    match (signature[0].as_str(), signature.len()) {
        ("__add__", 2) => format!("{} + {} # Only available in math mode", path.join(":"), signature[1]),
        ("__sub__", 2) => format!("{} - {} # Only available in math mode", path.join(":"), signature[1]),
        ("__mul__", 2) => format!("{} * {} # Only available in math mode", path.join(":"), signature[1]),
        ("__div__", 2) => format!("{} / {} # Only available in math mode", path.join(":"), signature[1]),
        ("__getitem__", 2) => format!("{}[{}]", path.join(":"), signature[1]),
        ("__setitem__", 3) => format!("{}[{}] = {}", path.join(":"), signature[1], signature[2]),
        ("match", 2) => format!("{} =~ {}", path.join(":"), signature[1]),
        ("not_match", 2) => format!("{} !~ {}", path.join(":"), signature[1]),
        _ => format!("{}:{}", path.join(":"), signature.join(" "))
    }
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

            let mut argument_desciptions = quote! {};

            for field in &mut s.fields {
                let mut default_value = None;
                let mut is_unnamed_target = false;
                let mut is_named_target = false;
                let mut allowed_values = None;
                let mut description = None;
                let mut completion_command = quote! {None};
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
                        } else if call_is_named(attr, "custom_completion") {
                            let name = call_value(attr)?;
                            completion_command = quote! {Some(#name)};
                        } else if call_is_named(attr, "description") {
                            description = Some(unescape(&(call_literal(attr)?.to_string())));
                        }
                    }
                }
                field.attrs = Vec::new();
                let name = &field.ident.clone().unwrap();
                let name_string = Literal::string(&name.to_string());

                let type_data =
                    Signature::new(
                        &field.ty,
                        name,
                        default_value.clone(),
                        is_unnamed_target,
                        allowed_values)?.type_data()?;

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

                let allowed_values = type_data.allowed_values;

                argument_desciptions = quote! {
                    #argument_desciptions
                    crate::lang::command::ArgumentDescription {
                        name: #name_string.to_string(),
                        value_type: #crush_internal_type,
                        allowed: #allowed_values,
                        description: None,
                        complete: #completion_command,
                        named: false,
                        unnamed: false,
                    },
                };
            }

            if let Some(example) = metadata.example {
                long_description.push("Example".to_string());
                long_description.push(example);
            }

            let has_path = metadata.path.len() > 0;
            let signature_literal = Literal::string(&signature.join(" "));
            let full_signature_literal = if has_path {
                Literal::string(&generate_signature(&metadata.path, signature))
            } else {
                signature_literal.clone()
            };


            let long_description = if !long_description.is_empty() {
                let mut s = "    ".to_string();
                s.push_str(&long_description.join("\n\n    "));
                let text = Literal::string(&s);
                quote! {Some(#text) }
            } else {
                quote! {None}
            };

            let handler =
                if has_path {
                    let mut vec_stream = TokenStream::new();
                    vec_stream.extend(metadata.path.iter().flat_map(|e| vec![TokenTree::Literal(Literal::string(e)),
                                                                             TokenTree::Punct(Punct::new(',', Spacing::Alone))]));
                    let path = TokenTree::Group(Group::new(Delimiter::None, vec_stream));
                    quote! {

            #[allow(unused_parens)] // TODO: don't emit unnecessary parenthesis in the first place
            impl #struct_name {
                pub fn declare(env: &mut crate::lang::state::scope::ScopeLoader) -> crate::lang::errors::CrushResult <()> {
                    env.declare_command(
                        #command_name,
                        #command_invocation,
                        #can_block,
                        #full_signature_literal,
                        #description,
                        #long_description,
                        #output,
                        vec![
                            #argument_desciptions
                        ],
                    )
                }

                pub fn declare_method(env: &mut ordered_map::OrderedMap<std::string::String, crate::lang::command::Command>) {
                    let mut full = vec!["global", #path];
                    full.push(#command_name);
                    env.insert(#command_name.to_string(),
                        <dyn crate::lang::command::CrushCommand>::command(
                            #command_invocation,
                            #can_block,
                            full,
                            #full_signature_literal,
                            #description,
                            #long_description,
                            #output,
                            [#argument_desciptions],
                        )
                    );
                }

                #[allow(unreachable_patterns)]
                pub fn parse(_arguments: Vec<crate::lang::argument::Argument>, _printer: &crate::lang::printer::Printer) -> crate::lang::errors::CrushResult < # struct_name > {
                    use std::convert::TryFrom;
                    use std::ops::Deref;
                    # values
                    let mut _unnamed = std::collections::VecDeque::new();

                    for _arg in _arguments {
                        let _location = _arg.location;
                        match (_arg.argument_type.as_deref(), _arg.value) {
                            #named_matchers
                            #named_fallback
                            (None, _value) => _unnamed.push_back((_value, _arg.location)),
                            (Some(_name), _value) => return crate::lang::errors::argument_error(format!("Unknown argument name \"{}\"", _name), _location),
                        }
                    }

                    #unnamed_mutations

                    Ok( #struct_name { #assignments })
                }
            }}
                } else {
                    quote! {

            #[allow(unused_parens)] // TODO: don't emit unnecessary parenthesis in the first place
            impl #struct_name {
                pub fn declare(env: &mut crate::lang::state::scope::ScopeLoader) -> crate::lang::errors::CrushResult <()> {
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
                        <dyn crate::lang::command::CrushCommand>::command(
                            #command_invocation,
                            #can_block,
                            full,
                            #full_signature_literal,
                            #description,
                            #long_description,
                            #output,
                            [#argument_desciptions],
                        )
                    );
                }

                #[allow(unreachable_patterns)]
                pub fn parse(_arguments: Vec<crate::lang::argument::Argument>, _printer: &crate::lang::printer::Printer) -> crate::lang::errors::CrushResult < # struct_name > {
                    use std::convert::TryFrom;
                    use std::ops::Deref;
                    # values
                    let mut _unnamed = std::collections::VecDeque::new();

                    for _arg in _arguments {
                        let _location = _arg.location;
                        match (_arg.argument_type.as_deref(), _arg.value) {
                            #named_matchers
                            #named_fallback
                            (None, _value) => _unnamed.push_back((_value, _arg.location)),
                            (Some(_name), _value) => return crate::lang::errors::argument_error(format!("Unknown argument name \"{}\"", _name), _location),
                        }
                    }

                    #unnamed_mutations

                    Ok( #struct_name { #assignments })
                }
            }
            }
                };

            let mut output = s.to_token_stream();
            output.extend(handler.into_token_stream());
            if struct_name.to_string() == "AllowedValuesStringSignature" {
                println!("{}", output.to_string());
            }
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
