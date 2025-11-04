use crate::{Patch, PatchCollection, PatchMessage, Result, da::uart_port::UartPort};

pub mod uart_port;

/// DA patches
pub enum DAPatches<'a> {
    /// Force uart0 for the DA logs
    UartPort(UartPort<'a>),
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
        }
    }

    /// Target offset to patch
    pub fn offset(&self, bytes: &[u8]) -> Result<usize> {
        match self {
            Self::UartPort(p) => p.offset(bytes),
        }
    }

    /// Patch replacement code
    pub fn replacement(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        match self {
            Self::UartPort(p) => p.replacement(bytes),
        }
    }

    /// Message when the patch is applied
    #[must_use]
    pub fn on_success(&self) -> &'static str {
        match self {
            Self::UartPort(p) => Self::on_success_internal(p),
        }
    }

    /// Message when the patch is failed to apply
    #[must_use]
    pub fn on_failure(&self) -> &'static str {
        match self {
            Self::UartPort(p) => Self::on_failure_internal(p),
        }
    }
}

/// DA patches
pub struct DA;
impl<'a> PatchCollection<'a, DAPatches<'a>> for DA {
    fn security(
        _assembler: &'a crate::Assembler,
        _disassembler: &'a crate::Disassembler,
    ) -> Vec<DAPatches<'a>> {
        vec![]
    }

    fn hardcoded(
        assembler: &'a crate::Assembler,
        disassembler: &'a crate::Disassembler,
    ) -> Vec<DAPatches<'a>> {
        vec![DAPatches::UartPort(UartPort::new(assembler, disassembler))]
    }
}
