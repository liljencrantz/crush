use proc_macro2;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;
use std::fs;
use syn::spanned::Spanned;

fn test_finder_real(span: Span) -> Option<TokenStream> {
    let mut output = TokenStream::new();

    for maybe_entry in fs::read_dir("tests").ok()? {
        let entry = maybe_entry.ok()?;
        let name = entry.path().to_str()?.to_string();
        if name.ends_with(".crush") {
            let test_identifier = Ident::new(
                &format!(
                    "test_{}",
                    entry.file_name().to_str()?.trim_end_matches(".crush")
                ),
                span,
            );
            let test_filename_literal = Literal::string(&name);
            output.extend(quote!(
                #[test]
                fn #test_identifier() {
                    run_system_test(Path::new(#test_filename_literal));
                }
            ));
        }
    }
    Some(output)
}

#[proc_macro]
pub fn test_finder(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let span = TokenStream::from(input).span();
    proc_macro::TokenStream::from(test_finder_real(span).unwrap_or_else(|| {
        syn::Error::new(span, "Failed to generate system test list").to_compile_error()
    }))
}
