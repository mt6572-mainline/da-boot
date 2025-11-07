use darling::{FromDeriveInput, FromField, FromMeta};
use derive_more::IsVariant;
use syn::Ident;

macro_rules! all_none {
    ($($opt:expr),+ $(,)?) => {{
        $( matches!($opt, None) )&&+
    }};
}

macro_rules! all_some {
    ($($opt:expr),+ $(,)?) => {{
        $( matches!($opt, Some(_)) )&&+
    }};
}

macro_rules! overlap {
    ($($opt:expr),+ $(,)?) => {{
        let mut n = 0;
        $(
            if matches!($opt, Some(_)) {
                n += 1;
            }
        )+
        n != 1
    }};
}

macro_rules! err {
    ($msg:literal) => {{
        let at = proc_macro2::Span::call_site();
        Err(syn::Error::new(at, $msg))
    }};
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(protocol), supports(struct_named, struct_unit))]
pub(crate) struct DarlingProtocolArgs {
    command: Option<u8>,
    naked: Option<()>,
}

pub(crate) enum ProtocolKind {
    /// Preloader or DA command
    Command(u8),
    /// Raw struct
    Naked,
}

impl TryFrom<DarlingProtocolArgs> for ProtocolKind {
    type Error = syn::Error;

    fn try_from(value: DarlingProtocolArgs) -> Result<Self, Self::Error> {
        if all_some!(value.command, value.naked) {
            return err!("both command and naked are not supported");
        } else if all_none!(value.command, value.naked) {
            return err!("struct must be command or naked");
        }

        Ok(match value.command {
            Some(c) => Self::Command(c),
            None => Self::Naked,
        })
    }
}

#[derive(Debug, FromMeta, IsVariant)]
pub(crate) enum AckType {
    /// Wait for ack and echo back
    RxThenTx,
    /// Send ack and wait for echo
    TxThenRx,
}

#[derive(Debug, FromField)]
#[darling(attributes(protocol))]
pub(crate) struct DarlingProtocolField {
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
    #[darling(default)]
    ack: Option<AckType>,
    #[darling(default)]
    always: Option<u32>,
    #[darling(default)]
    getter: Option<()>,
}

#[derive(Clone, IsVariant)]
pub(crate) enum RxType {
    Status(u16),
    Size(Ident),
    None,
}

#[derive(IsVariant)]
pub(crate) enum TxType {
    Always(u32),
    None,
}

#[derive(IsVariant)]
pub(crate) enum FieldType {
    Tx(TxType),
    Rx { ty: RxType, getter: bool },
    Echo,
    Ack(AckType),
}

impl TryFrom<DarlingProtocolField> for FieldType {
    type Error = syn::Error;

    fn try_from(value: DarlingProtocolField) -> Result<Self, Self::Error> {
        if all_some!(value.tx, value.rx, value.echo, value.ack) {
            return err!("specify only tx or rx or echo");
        } else if all_none!(value.tx, value.rx, value.echo, value.ack) {
            return err!("dummy fields are not allowed for the protocol structs");
        } else if overlap!(value.tx, value.rx, value.echo, value.ack) {
            return err!("field must be tx or rx or echo or ack");
        }

        if all_some!(value.tx, value.status) {
            return err!("tx field cannot be a status");
        } else if all_some!(value.tx, value.size) {
            return err!("only rx field can have size");
        } else if all_some!(value.tx, value.getter) {
            return err!("only rx field can have getter");
        } // other sanity checks are todo

        if value.tx.is_some() {
            Ok(if value.always.is_some() {
                Self::Tx(TxType::Always(value.always.unwrap()))
            } else {
                Self::Tx(TxType::None)
            })
        } else if value.rx.is_some() {
            if all_some!(value.status, value.size) {
                return err!("status and value must not overlap for the rx field");
            }

            let ty = if value.status.is_some() {
                RxType::Status(value.status.unwrap())
            } else if value.size.is_some() {
                RxType::Size(value.size.unwrap())
            } else {
                RxType::None
            };

            Ok(Self::Rx {
                ty,
                getter: value.getter.is_some(),
            })
        } else if value.echo.is_some() {
            Ok(Self::Echo)
        } else if value.ack.is_some() {
            Ok(Self::Ack(value.ack.unwrap()))
        } else {
            unreachable!()
        }
    }
}
