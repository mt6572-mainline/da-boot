use darling::{FromDeriveInput, FromField};
use derive_ctor::ctor;
use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, Type, parse_macro_input};

use crate::structs::{DarlingProtocolArgs, DarlingProtocolField, FieldType, ProtocolKind, RxType, TxType};

mod structs;

#[derive(Clone)]
enum CodegenType {
    U8,
    U16,
    U32,
    Vec,
    Slice,
}

impl CodegenType {
    fn try_from_internal<'a, T: PartialEq<&'a str>>(value: T) -> Option<Self> {
        if value == "u8" {
            Some(Self::U8)
        } else if value == "u16" {
            Some(Self::U16)
        } else if value == "u32" {
            Some(Self::U32)
        } else if value == "Vec" {
            Some(Self::Vec)
        } else if value == "[u8]" {
            Some(Self::Slice)
        } else {
            None
        }
    }
}

impl TryFrom<&str> for CodegenType {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from_internal(value).ok_or(())
    }
}

impl TryFrom<&Ident> for CodegenType {
    type Error = ();

    fn try_from(value: &Ident) -> Result<Self, Self::Error> {
        Self::try_from_internal(value).ok_or(())
    }
}

#[derive(ctor)]
struct Field {
    ident: Ident,
    enum_ty: FieldType,
    ty: Type,
}

struct Codegen {
    ty: CodegenType,
    ident: Ident,
    self_prefix: bool,
    tokenstream: proc_macro2::TokenStream,
}
impl Codegen {
    pub fn new(ty: CodegenType, ident: Ident, self_prefix: bool) -> Self {
        Self {
            ty,
            ident,
            self_prefix,
            tokenstream: proc_macro2::TokenStream::new(),
        }
    }

    pub fn load(mut self) -> Self {
        let from = &self.ident;
        let stream = if self.self_prefix {
            quote! { let tmp = self.#from; }
        } else {
            quote! { let tmp = #from; }
        };

        self.tokenstream.extend(stream);

        self
    }

    pub fn store(mut self) -> Self {
        let to = &self.ident;
        let stream = if self.self_prefix {
            quote! { self.#to = tmp; }
        } else {
            quote! { #to = tmp; }
        };

        self.tokenstream.extend(stream);

        self
    }

    pub fn status(mut self, status: u16) -> Self {
        let stream = quote! {
            if (tmp as u16) != (#status as u16) {
                return Err(Error::InvalidStatus(#status as u16, tmp as u16));
            }
        };

        self.tokenstream.extend(stream);

        self
    }

    pub fn tx(mut self) -> Self {
        let stream = match self.ty {
            CodegenType::U8 | CodegenType::U16 | CodegenType::U32 => Some(quote! {
                port.simple_write(tmp)?;

            }),
            CodegenType::Vec => Some(quote! {
                port.write_all(&tmp)?;
            }),
            CodegenType::Slice => Some(quote! {
                port.write_all(tmp)?;
            }),
        }
        .unwrap();

        self.tokenstream.extend(stream);

        self
    }

    pub fn rx(mut self) -> Self {
        let stream = match self.ty {
            CodegenType::U8 => Some(quote! {
                let tmp = port.read_u8()?;
            }),
            CodegenType::U16 => Some(quote! {
                let tmp = port.read_u16()?;
            }),
            CodegenType::U32 => Some(quote! {
                let tmp = port.read_u32()?;
            }),
            _ => None,
        }
        .unwrap();

        self.tokenstream.extend(stream);

        self
    }

    pub fn echo_status(mut self) -> Self {
        let ident = &self.ident;
        let ident = if self.self_prefix {
            quote! { self.#ident }
        } else {
            quote! { #ident }
        };
        let stream = quote! {
            if tmp != #ident {
                return Err(Error::InvalidEchoData(#ident as u32, tmp as u32));
            }

        };

        self.tokenstream.extend(stream);

        self
    }

    pub fn push(mut self, stream: proc_macro2::TokenStream) -> Self {
        self.tokenstream.extend(stream);

        self
    }

    pub fn finalize(self) -> proc_macro2::TokenStream {
        self.tokenstream
    }
}

#[proc_macro_derive(Protocol, attributes(protocol))]
pub fn da_legacy(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let struct_generics = &input.generics;

    let args = match ProtocolKind::try_from(match DarlingProtocolArgs::from_derive_input(&input) {
        Ok(v) => v,
        Err(e) => return e.write_errors().into(),
    }) {
        Ok(v) => v,
        Err(e) => return e.into_compile_error().into(),
    };

    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(data) => Some(data.named),
            Fields::Unit => None,
            _ => {
                return syn::Error::new_spanned(struct_name, "Only named fields are supported").to_compile_error().into();
            }
        },
        _ => {
            return syn::Error::new_spanned(struct_name, "Only structs are supported").to_compile_error().into();
        }
    };

    let fields = match fields {
        Some(fields) => fields
            .into_iter()
            .filter_map(|f| {
                let attrs = DarlingProtocolField::from_field(&f).map_err(darling::Error::write_errors).unwrap();
                let ident = f.ident.unwrap();
                let ty = f.ty;
                let enum_ty = FieldType::try_from(attrs).unwrap();

                Some(Field::new(ident, enum_ty, ty))
            })
            .collect::<Vec<_>>(),
        None => vec![],
    };

    // For TX and echo fields generate a constructor
    let tx_echo_fields = fields
        .iter()
        .filter(|f| match &f.enum_ty {
            FieldType::Tx(t) if t.is_none() => true,
            FieldType::Echo => true,
            FieldType::Ack(_) => true,
            _ => false,
        })
        .collect::<Vec<_>>();
    let ctor = if tx_echo_fields.is_empty() {
        quote! {
            pub fn new() -> Self {
                Self { ..Default::default() }
            }
        }
    } else {
        let args = tx_echo_fields
            .iter()
            .map(|f| {
                let ident = &f.ident;
                let ty = &f.ty;
                quote! { #ident: #ty }
            })
            .collect::<Vec<_>>();
        let self_args = tx_echo_fields
            .iter()
            .map(|f| {
                let ident = &f.ident;
                quote! {#ident}
            })
            .collect::<Vec<_>>();
        quote! {
            /// Refer to fields marked with `#[protocol(tx)]` and `#[protocol(echo)]` for the explanation of the constructor arguments
            pub fn new(#(#args,)*) -> Self {
                Self { #(#self_args,)* ..Default::default() }
            }
        }
    };

    let methods = fields
        .iter()
        .filter_map(|f| match &f.ty {
            Type::Path(ty) => match &f.enum_ty {
                FieldType::Rx { ty: rx_ty, getter } if *getter => {
                    let ident = &f.ident;
                    Some(if rx_ty.is_size() {
                        let as_ident = format_ident!("as_{ident}");
                        let into_ident = format_ident!("into_{ident}");
                        quote! {
                            /// Extract field from the struct
                            pub fn #into_ident(self) -> #ty {
                                self.#ident
                            }

                            /// Get a reference to the field
                            pub fn #as_ident(&self) -> &#ty {
                                &self.#ident
                            }
                        }
                    } else {
                        quote! {
                            pub fn #ident(&self) -> #ty {
                                self.#ident
                            }
                        }
                    })
                }
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();

    let code = fields
        .into_iter()
        .map(|f| match &f.ty {
            Type::Path(ty) => {
                let ty = CodegenType::try_from(&ty.path.segments.last().unwrap().ident).unwrap();
                match f.enum_ty {
                    FieldType::Tx(tx_ty) => {
                        let ident = &f.ident;
                        let default = match tx_ty {
                            TxType::Always(v) => Some(quote! { self.#ident = #v.try_into().map_err(|e| Error::Custom(format!("Int conversion failed, this shouldn't happen unless the codegen struct is messed up: {e}").into()))?; }),
                            TxType::None => None,
                        };

                        let code = Codegen::new(ty, f.ident, true).load().tx().finalize();
                        quote! {
                            #default
                            #code
                        }
                    }
                    FieldType::Rx { ty: rx_ty, .. } => {
                        let ident = f.ident.clone();
                        let code = Codegen::new(ty, f.ident.clone(), true);
                        let code = if let RxType::Size(size) = rx_ty.clone() {
                            let inner = CodegenType::try_from(f.ty.to_token_stream().to_string().replace(' ', "").replace("Vec<", "").replace('>', "").as_str()).unwrap();
                            let inner_code = Codegen::new(inner, f.ident, true).rx().finalize();
                            code.push(quote! {
                                for i in 0..self.#size {
                                    #inner_code
                                    self.#ident.push(tmp);
                                }
                            })
                        } else {
                            code.rx()
                        };
                        let code = if let RxType::Status(status) = rx_ty { code.status(status) } else { code };
                        if rx_ty.is_size() { code.finalize() } else { code.store().finalize() }
                    }
                    FieldType::Echo => Codegen::new(ty, f.ident, true).load().tx().rx().echo_status().store().finalize(),
                    FieldType::Ack(ack_ty) => {
                        let code = Codegen::new(ty, f.ident.clone(), true);
                        if ack_ty.is_tx_then_rx() {
                            code.load().tx().rx().echo_status().store()
                        } else {
                            code.rx().store().load().tx()
                        }.finalize()
                    }
                }
            }
            Type::Reference(ty) => Codegen::new(
                CodegenType::try_from(ty.elem.to_token_stream().to_string().trim().replace('&', "").as_str()).unwrap(),
                f.ident,
                true,
            )
            .load()
            .tx()
            .finalize(),
            _ => panic!(":("),
        })
        .collect::<Vec<_>>();

    let command = if let ProtocolKind::Command(command) = args {
        let ident = format_ident!("command");
        let command_code = Codegen::new(CodegenType::U8, ident.clone(), false).load().tx().rx().echo_status().finalize();
        quote! {
            let #ident = #command;
            #command_code
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        impl #struct_generics #struct_name #struct_generics {
            #ctor
            #(#methods)*

            /// Runs the command
            pub fn run(&mut self, port: &mut crate::Port) -> crate::Result<()> {
                use da_protocol::{SimpleRead, SimpleWrite};
                #command
                #(#code;)*

                Ok(())
            }
        }
    };

    TokenStream::from(expanded)
}
