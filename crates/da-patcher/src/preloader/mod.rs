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
    /// sec_region_check function patch
    SecRegionCheck(SecRegionCheck<'a>),
    /// send_da command patch
    SendDA(SendDA<'a>),
    /// jump_da command patch
    JumpDA(JumpDA<'a>),
    /// jump_da boot argument address patch
    DABootArgument(DABootArgument<'a>),
    /// seclib_sec_usbdl_enabled function patch
    DAA(DAA<'a>),
}

impl<'a> PreloaderPatches<'a> {
    #[inline]
    fn patch_internal<T: Patch<'a>>(p: &T, bytes: &mut [u8]) -> Result<()> {
        p.patch(bytes)
    }

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
            Self::SecRegionCheck(p) => Self::patch_internal(p, bytes),
            Self::SendDA(p) => Self::patch_internal(p, bytes),
            Self::JumpDA(p) => Self::patch_internal(p, bytes),
            Self::DABootArgument(p) => Self::patch_internal(p, bytes),
            Self::DAA(p) => Self::patch_internal(p, bytes),
        }
    }

    /// Message when the patch is applied
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
