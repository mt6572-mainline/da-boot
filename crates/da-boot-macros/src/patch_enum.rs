use crate::compile_err;
use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, parse_macro_input};

pub fn patch_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = &input.ident;

    let fields = match input.data {
        syn::Data::Enum(e) => e.variants,
        _ => return compile_err!(enum_name, "expected enum"),
    };

    let methods = fields
        .into_iter()
        .map(|f| {
            let ident = &f.ident;
            let snake_ident = format_ident!("{}", f.ident.to_string().to_case(Case::Snake));
            quote! {
                pub fn #snake_ident(assembler: &'a crate::Assembler, disassembler: &'a crate::Disassembler) -> Self {
                    Self::#ident(#ident::new(assembler, disassembler))
                }
            }
        })
        .collect::<Vec<_>>();

    TokenStream::from(quote! {
        impl<'a> #enum_name<'a> {
            #(#methods)*
        }
    })
}
