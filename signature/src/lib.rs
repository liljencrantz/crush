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
                        "bool" => "bool",
                        "char" => "char",
                        "f64" => "f64",
                        "ValueType" => "ValueType",
                        "PathBuf" => "PathBuf",
                        "OrderedStringMap" => "OrderedStringMap",
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

fn call_value(attr: &Attribute) -> SignatureResult<TokenTree> {
    let values = call_values(attr)?;
    if values.len() == 1 {
        Ok(values[0].clone())
    } else {
        fail!(attr.span(), "Expected exactly one literal")
    }
}

fn simple_type_to_value_name(simple_type: &str) -> &str {
    match simple_type {
        "String" => "String",
        "bool" => "Bool",
        "i128" => "Integer",
        "ValueType" => "Type",
        "f64" => "Float",
        "char" => "String",
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
        "i128" | "bool" | "String" | "char" | "ValueType" | "f64" => {
            if !args.is_empty() {
                fail!(ty.span(), "This type can't be paramterizised")
            } else {
                let native_type = Ident::new(type_name, ty.span());
                let mutator = simple_type_to_mutator(type_name, &allowed_values_name);
                let value_type = Ident::new(simple_type_to_value_name(type_name), ty.span());
                Ok(TypeData {
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
                    mappings: quote! {(Some(#name_literal), Value::#value_type(value)) => #name = Some(#mutator),},
                    unnamed_mutate:
                    match default {
                        None => {
                            Some(quote! {
if #name.is_none() {
    if let Some(Value::#value_type(value)) = _unnamed.pop_front() {
        #name = Some(#mutator);
    } else {
        return crate::lang::errors::argument_error(format!("Expected argument {} to be of type {}", #name_literal, #type_name).as_str());
    }
}
                            })
                        }
                        Some(def) => {
                            Some(quote! {
if #name.is_none() {
    match _unnamed.pop_front() {
        Some(Value::#value_type(value)) => #name = Some(#mutator),
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

        "Vec" => {
            if allowed_values.is_some() {
                return fail!(ty.span(), "Vactors can't have restricted values");
            }
            if args.len() != 1 {
                fail!(ty.span(), "Vec needs exactly one parameter")
            } else if args[0] == "PathBuf" {
                Ok(TypeData {
                    initialize: quote! { let mut #name = Vec::new(); },
                    mappings: quote! { (Some(#name_literal), value) => value.file_expand(&mut #name, printer)?, },
                    unnamed_mutate: if is_unnamed_target {
                        Some(quote! {
                            while !_unnamed.is_empty() {
                                _unnamed.pop_front().unwrap().file_expand(&mut #name, printer)?;
                            }
                        })
                    } else { None },
                    assign: quote! { #name, },
                })
            } else {
                let mutator = simple_type_to_mutator(args[0], &None);
                let dump_all = Ident::new(simple_type_dump_list(args[0]), ty.span().clone());
                let value_type = Ident::new(simple_type_to_value_name(args[0]), ty.span().clone());

                Ok(TypeData {
                    initialize: quote! { let mut #name = Vec::new(); },
                    mappings: quote! {
                        (Some(#name_literal), Value::#value_type(value)) => #name.push(#mutator),
                        (Some(#name_literal), Value::List(value)) => value.#dump_all(&mut #name)?,
                    },
                    unnamed_mutate: if is_unnamed_target {
                        Some(quote! {
                            while !_unnamed.is_empty() {
                                if let Some(Value::#value_type(value)) = _unnamed.pop_front() {
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
                let value_type = Ident::new(simple_type_to_value_name(args[0]), ty.span().clone());

                Ok(TypeData {
                    initialize: quote! { let mut #name = crate::lang::ordered_string_map::OrderedStringMap::new(); },
                    mappings: quote! { (Some(name), Value::#value_type(value)) => #name.insert(name.to_string(), #mutator), },
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
                let value_type = Ident::new(simple_type_to_value_name(args[0]), ty.span().clone());

                Ok(TypeData {
                    initialize: quote! { let mut #name = None; },
                    mappings: quote! { (Some(#name_literal), Value::#value_type(value)) => #name = Some(#mutator), },
                    unnamed_mutate: Some(quote_spanned! { ty.span() =>
                            if #name.is_none() {
                                match _unnamed.pop_front() {
                                    None => {}
                                    Some(Value::#value_type(value)) => #name = Some(#mutator),
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

fn signature_real(input: TokenStream) -> SignatureResult<TokenStream> {
    let root: syn::Item = syn::parse2(input).expect("Invalid syntax tree");
    match root {
        Item::Struct(mut s) => {
            let mut named_matchers = proc_macro2::TokenStream::new();
            let mut values = proc_macro2::TokenStream::new();
            let mut unnamed_mutations = proc_macro2::TokenStream::new();
            let mut assignments = proc_macro2::TokenStream::new();
            let mut named_fallback = proc_macro2::TokenStream::new();
            let mut had_unnamed_target = false;
            let struct_name = s.ident.clone();
            for field in &mut s.fields {
                let mut default_value = None;
                let mut is_unnamed_target = false;
                let mut is_named_target = false;
                let mut allowed_values = None;
                if !field.attrs.is_empty() {
                    for attr in &field.attrs {
                        if call_is_default(attr) {
                            default_value = Some(call_value(attr)?)
                        }
                        if call_is_named(attr, "unnamed") {
                            is_unnamed_target = true;
                        }
                        if call_is_named(attr, "named") {
                            is_named_target = true;
                        }
                        if call_is_named(attr, "values") {
                            allowed_values = Some(call_literals(attr)?);
                        }
                    }
                }
                field.attrs = Vec::new();
                let name = &field.ident.clone().unwrap();
                let type_data = type_to_value(&field.ty, name, default_value.clone(), is_unnamed_target, allowed_values)?;
                let initialize = type_data.initialize;
                let mappings = type_data.mappings;
                values.extend(initialize);

                if is_named_target {
                    named_fallback.extend(mappings)
                } else {
                    named_matchers.extend(mappings);
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

            let handler = quote! {

impl crate::lang::argument::ArgumentHandler for #struct_name {
    fn parse(arguments: Vec<crate::lang::argument::Argument>, printer: &crate::lang::printer::Printer) -> crate::lang::errors::CrushResult < # struct_name > {
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
pub fn signature(_metadata: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match signature_real(TokenStream::from(input)) {
        Ok(res) | Err(res) => {
            proc_macro::TokenStream::from(res)
        }
    }
}

