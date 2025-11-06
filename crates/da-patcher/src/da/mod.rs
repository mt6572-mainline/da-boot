use crate::{
    Patch, PatchCollection, PatchMessage, Result,
    da::{hash::Hash, uart_port::UartPort},
};

pub mod hash;
pub mod uart_port;

/// DA patches
pub enum DAPatches<'a> {
    /// Force uart0 for the DA logs
    UartPort(UartPort<'a>),
    /// Disable hash check in the DA1
    Hash(Hash<'a>),
}

impl<'a> DAPatches<'a> {
    #[inline]
    fn on_success_internal<T: PatchMessage>(_p: &T) -> &'static str {
        T::on_success()
    }

    #[inline]
    fn on_failure_internal<T: PatchMessage>(_p: &T) -> &'static str {
        T::on_failure()
    }

    /// Apply the patch
    pub fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        match self {
            Self::UartPort(p) => p.patch(bytes),
            Self::Hash(p) => p.patch(bytes),
        }
    }

    /// Target offset to patch
    pub fn offset(&self, bytes: &[u8]) -> Result<usize> {
        match self {
            Self::UartPort(p) => p.offset(bytes),
            Self::Hash(p) => p.offset(bytes),
        }
    }

    /// Patch replacement code
    pub fn replacement(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        match self {
            Self::UartPort(p) => p.replacement(bytes),
            Self::Hash(p) => p.replacement(bytes),
        }
    }

    /// Message when the patch is applied
    #[must_use]
    pub fn on_success(&self) -> &'static str {
        match self {
            Self::UartPort(p) => Self::on_success_internal(p),
            Self::Hash(p) => Self::on_success_internal(p),
        }
    }

    /// Message when the patch is failed to apply
    #[must_use]
    pub fn on_failure(&self) -> &'static str {
        match self {
            Self::UartPort(p) => Self::on_failure_internal(p),
            Self::Hash(p) => Self::on_failure_internal(p),
        }
    }
}

/// DA patches
pub struct DA;
impl<'a> PatchCollection<'a, DAPatches<'a>> for DA {
    fn security(
        assembler: &'a crate::Assembler,
        disassembler: &'a crate::Disassembler,
    ) -> Vec<DAPatches<'a>> {
        vec![DAPatches::Hash(Hash::new(assembler, disassembler))]
    }

    fn hardcoded(
        assembler: &'a crate::Assembler,
        disassembler: &'a crate::Disassembler,
    ) -> Vec<DAPatches<'a>> {
        vec![DAPatches::UartPort(UartPort::new(assembler, disassembler))]
    }
}
