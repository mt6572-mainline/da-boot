use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, parse_macro_input};

#[proc_macro_attribute]
pub fn alternatives(attr: TokenStream, item: TokenStream) -> TokenStream {
    let tokens = item.to_string();
    let input = parse_macro_input!(item as ItemFn);

    let log_block = if attr.is_empty() {
        None
    } else {
        let s = match syn::parse::<LitStr>(attr.clone()) {
            Ok(s) => s,
            Err(_) => {
                return syn::Error::new_spanned(tokens, "invalid fmt")
                    .into_compile_error()
                    .into();
            }
        };

        Some(quote! {
            uart_printfln!(#s, value);
        })
    };

    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;

    if !sig.inputs.is_empty() {
        return syn::Error::new_spanned(&sig.inputs, "args must be empty")
            .to_compile_error()
            .into();
    }

    let ret_ty = match &sig.output {
        syn::ReturnType::Type(_, ty) => ty,
        syn::ReturnType::Default => {
            return syn::Error::new_spanned(&sig, "add ret")
                .to_compile_error()
                .into();
        }
    };

    quote! {
        #vis #sig {
            static CELL: Cell<#ret_ty> = Cell::new();

            *CELL.get_or_init(|| {
                let value = { #block };
                #log_block
                value
            })
        }
    }
    .into()
}
