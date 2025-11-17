use crate::{
    PatchCollection,
    da::{hash::Hash, uart_port::UartPort},
};
use da_boot_macros::PatchEnum;
use enum_dispatch::enum_dispatch;

pub mod hash;
pub mod uart_port;

/// DA patches
#[enum_dispatch(Patch)]
#[derive(PatchEnum)]
pub enum DAPatches<'a> {
    /// Force uart0 for the DA logs
    UartPort(UartPort<'a>),
    /// Disable hash check in the DA1
    Hash(Hash<'a>),
}

impl DAPatches<'_> {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::UartPort(_) => "DA UART1 -> UART0",
            Self::Hash(_) => "DA1 hash check",
        }
    }
}

pub struct DA;
impl<'a> PatchCollection<'a, DAPatches<'a>> for DA {
    fn security(
        assembler: &'a crate::Assembler,
        disassembler: &'a crate::Disassembler,
    ) -> Vec<DAPatches<'a>> {
        vec![DAPatches::hash(assembler, disassembler)]
    }

    fn hardcoded(
        assembler: &'a crate::Assembler,
        disassembler: &'a crate::Disassembler,
    ) -> Vec<DAPatches<'a>> {
        vec![DAPatches::uart_port(assembler, disassembler)]
    }
}
