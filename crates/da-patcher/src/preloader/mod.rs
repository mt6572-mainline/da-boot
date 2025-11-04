use crate::{
    Patch, PatchCollection, PatchMessage, Result,
    preloader::{
        da_argument::DABootArgument, daa::DAA, jump_da::JumpDA, sec_region_check::SecRegionCheck,
        send_da::SendDA,
    },
};

pub mod da_argument;
pub mod daa;
pub mod jump_da;
pub mod sec_region_check;
pub mod send_da;

/// Preloader patches
pub enum PreloaderPatches<'a> {
    /// `sec_region_check` function patch
    SecRegionCheck(SecRegionCheck<'a>),
    /// `send_da` command patch
    SendDA(SendDA<'a>),
    /// `jump_da` command patch
    JumpDA(JumpDA<'a>),
    /// `jump_da` boot argument address patch
    DABootArgument(DABootArgument<'a>),
    /// `seclib_sec_usbdl_enabled` function patch
    DAA(DAA<'a>),
}

impl<'a> PreloaderPatches<'a> {
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
            Self::SecRegionCheck(p) => p.patch(bytes),
            Self::SendDA(p) => p.patch(bytes),
            Self::JumpDA(p) => p.patch(bytes),
            Self::DABootArgument(p) => p.patch(bytes),
            Self::DAA(p) => p.patch(bytes),
        }
    }

    /// Target offset to patch
    pub fn offset(&self, bytes: &[u8]) -> Result<usize> {
        match self {
            Self::SecRegionCheck(p) => p.offset(bytes),
            Self::SendDA(p) => p.offset(bytes),
            Self::JumpDA(p) => p.offset(bytes),
            Self::DABootArgument(p) => p.offset(bytes),
            Self::DAA(p) => p.offset(bytes),
        }
    }

    /// Patch replacement code
    pub fn replacement(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        match self {
            Self::SecRegionCheck(p) => p.replacement(bytes),
            Self::SendDA(p) => p.replacement(bytes),
            Self::JumpDA(p) => p.replacement(bytes),
            Self::DABootArgument(p) => p.replacement(bytes),
            Self::DAA(p) => p.replacement(bytes),
        }
    }

    /// Message when the patch is applied
    #[must_use]
    pub fn on_success(&self) -> &'static str {
        match self {
            Self::SecRegionCheck(p) => Self::on_success_internal(p),
            Self::SendDA(p) => Self::on_success_internal(p),
            Self::JumpDA(p) => Self::on_success_internal(p),
            Self::DABootArgument(p) => Self::on_success_internal(p),
            Self::DAA(p) => Self::on_success_internal(p),
        }
    }

    /// Message when the patch is failed to apply
    #[must_use]
    pub fn on_failure(&self) -> &'static str {
        match self {
            Self::SecRegionCheck(p) => Self::on_failure_internal(p),
            Self::SendDA(p) => Self::on_failure_internal(p),
            Self::JumpDA(p) => Self::on_failure_internal(p),
            Self::DABootArgument(p) => Self::on_failure_internal(p),
            Self::DAA(p) => Self::on_failure_internal(p),
        }
    }
}

/// Preloader patches
pub struct Preloader;
impl<'a> PatchCollection<'a, PreloaderPatches<'a>> for Preloader {
    fn security(
        assembler: &'a crate::Assembler,
        disassembler: &'a crate::Disassembler,
    ) -> Vec<PreloaderPatches<'a>> {
        vec![
            PreloaderPatches::SecRegionCheck(SecRegionCheck::new(assembler, disassembler)),
            PreloaderPatches::DAA(DAA::new(assembler, disassembler)),
        ]
    }

    fn hardcoded(
        assembler: &'a crate::Assembler,
        disassembler: &'a crate::Disassembler,
    ) -> Vec<PreloaderPatches<'a>> {
        vec![
            PreloaderPatches::SendDA(SendDA::new(assembler, disassembler)),
            PreloaderPatches::DABootArgument(DABootArgument::new(assembler, disassembler)),
            PreloaderPatches::JumpDA(JumpDA::new(assembler, disassembler)),
        ]
    }
}
