use darling::{FromDeriveInput, FromField};
use derive_ctor::ctor;
use derive_more::IsVariant;
use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, Type, parse_macro_input};

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(protocol), supports(struct_named, struct_unit))]
struct ProtocolArgs {
    command: Option<u8>,
}

#[derive(Debug, FromField)]
#[darling(attributes(protocol))]
struct ProtocolField {
    #[darling(default)]
    tx: Option<()>,
    #[darling(default)]
    rx: Option<()>,
    #[darling(default)]
    echo: Option<()>,
    #[darling(default)]
    status: Option<u16>,
    #[darling(default)]
    size: Option<Ident>,
}

#[derive(IsVariant)]
enum FieldType {
    Tx,
    Rx,
    Echo,
}

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
    status: Option<u16>,
    vec_recv_size: Option<Ident>,
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
            if tmp != #status {
                return Err(Error::InvalidStatus(#status as u16, tmp as u16));
            }
        };

        self.tokenstream.extend(stream);

        self
    }

    pub fn tx(mut self) -> Self {
        let stream = match self.ty {
            CodegenType::U8 | CodegenType::U16 | CodegenType::U32 => Some(quote! {
                port.write_all(&tmp.to_be_bytes())?;

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
                let mut buf = [0; 1];
                port.read_exact(&mut buf)?;
                let tmp = u8::from_be_bytes(buf);
            }),
            CodegenType::U16 => Some(quote! {
                let mut buf = [0; 2];
                port.read_exact(&mut buf)?;
                let tmp = u16::from_be_bytes(buf);
            }),
            CodegenType::U32 => Some(quote! {
                let mut buf = [0; 4];
                port.read_exact(&mut buf)?;
                let tmp = u32::from_be_bytes(buf);
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

    let args = match ProtocolArgs::from_derive_input(&input) {
        Ok(val) => val,
        Err(e) => return e.write_errors().into(),
    };

    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(data) => Some(data.named),
            Fields::Unit => None,
            _ => {
                return syn::Error::new_spanned(struct_name, "Only named fields are supported")
                    .to_compile_error()
                    .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(struct_name, "Only structs are supported")
                .to_compile_error()
                .into();
        }
    };

    let fields = match fields {
        Some(fields) => fields
            .into_iter()
            .filter_map(|f| {
                let attrs = ProtocolField::from_field(&f)
                    .map_err(darling::Error::write_errors)
                    .unwrap();
                let ident = f.ident.unwrap();
                let ty = f.ty;
                let enum_ty = if attrs.tx.is_some() {
                    Some(FieldType::Tx)
                } else if attrs.rx.is_some() {
                    Some(FieldType::Rx)
                } else if attrs.echo.is_some() {
                    Some(FieldType::Echo)
                } else {
                    None
                }?;

                Some(Field::new(ident, enum_ty, ty, attrs.status, attrs.size))
            })
            .collect::<Vec<_>>(),
        None => vec![],
    };

    // For TX and echo fields generate a constructor
    let tx_echo_fields = fields
        .iter()
        .filter(|f| f.enum_ty.is_tx() || f.enum_ty.is_echo())
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

    let code = fields
        .into_iter()
        .map(|f| match &f.ty {
            Type::Path(ty) => {
                let ty = CodegenType::try_from(&ty.path.segments.last().unwrap().ident).unwrap();
                match f.enum_ty {
                    FieldType::Tx => Codegen::new(ty, f.ident, true).load().tx().finalize(),
                    FieldType::Rx => {
                        let ident = f.ident.clone();
                        let code = Codegen::new(ty, f.ident.clone(), true);
                        let code = if let Some(size) = f.vec_recv_size.clone() {
                            let inner = CodegenType::try_from(
                                f.ty.to_token_stream()
                                    .to_string()
                                    .replace(' ', "")
                                    .replace("Vec<", "")
                                    .replace('>', "")
                                    .as_str(),
                            )
                            .unwrap();
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
                        let code = if let Some(status) = f.status {
                            code.status(status)
                        } else {
                            code
                        };
                        if f.vec_recv_size.is_some() {
                            code.finalize()
                        } else {
                            code.store().finalize()
                        }
                    }
                    FieldType::Echo => Codegen::new(ty, f.ident, true)
                        .load()
                        .tx()
                        .rx()
                        .echo_status()
                        .store()
                        .finalize(),
                }
            }
            Type::Reference(ty) => Codegen::new(
                CodegenType::try_from(
                    ty.elem
                        .to_token_stream()
                        .to_string()
                        .trim()
                        .replace('&', "")
                        .as_str(),
                )
                .unwrap(),
                f.ident,
                true,
            )
            .load()
            .tx()
            .finalize(),
            _ => panic!(":("),
        })
        .collect::<Vec<_>>();

    let command = if let Some(command) = args.command {
        let ident = format_ident!("command");
        let command_code = Codegen::new(CodegenType::U8, ident.clone(), false)
            .load()
            .tx()
            .rx()
            .echo_status()
            .finalize();
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

            /// Runs the command
            pub fn run(&mut self, port: &mut crate::Port) -> crate::Result<()> {
                #command
                #(#code;)*

                Ok(())
            }
        }
    };

    TokenStream::from(expanded)
}
