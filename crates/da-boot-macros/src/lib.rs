use proc_macro::TokenStream;

mod patch_enum;
mod protocol;
mod structs;

macro_rules! compile_err {
    ($at:expr, $err:literal) => {
        syn::Error::new_spanned($at, $err).into_compile_error().into()
    };
}

pub(crate) use compile_err;

#[proc_macro_derive(Protocol, attributes(protocol))]
pub fn da_legacy(input: TokenStream) -> TokenStream {
    protocol::da_legacy(input)
}

#[proc_macro_derive(PatchEnum)]
pub fn patch_enum(input: TokenStream) -> TokenStream {
    patch_enum::patch_enum(input)
}
