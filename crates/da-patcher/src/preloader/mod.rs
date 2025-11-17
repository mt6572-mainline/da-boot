use crate::{
    PatchCollection,
    preloader::{
        da_argument::DABootArgument, daa::DAA, jump_da::JumpDA, sec_region_check::SecRegionCheck,
        send_da::SendDA,
    },
};
use da_boot_macros::PatchEnum;
use enum_dispatch::enum_dispatch;

pub mod da_argument;
pub mod daa;
pub mod jump_da;
pub mod sec_region_check;
pub mod send_da;

/// Preloader patches
#[enum_dispatch(Patch)]
#[derive(PatchEnum)]
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

impl PreloaderPatches<'_> {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::SecRegionCheck(_) => "sec_region_check",
            Self::SendDA(_) => "send_da",
            Self::JumpDA(_) => "jump_da",
            Self::DABootArgument(_) => "jump_da boot argument",
            Self::DAA(_) => "DAA",
        }
    }
}

pub struct Preloader;
impl<'a> PatchCollection<'a, PreloaderPatches<'a>> for Preloader {
    fn security(
        assembler: &'a crate::Assembler,
        disassembler: &'a crate::Disassembler,
    ) -> Vec<PreloaderPatches<'a>> {
        vec![
            PreloaderPatches::sec_region_check(assembler, disassembler),
            PreloaderPatches::daa(assembler, disassembler),
        ]
    }

    fn hardcoded(
        assembler: &'a crate::Assembler,
        disassembler: &'a crate::Disassembler,
    ) -> Vec<PreloaderPatches<'a>> {
        vec![
            PreloaderPatches::da_boot_argument(assembler, disassembler),
            PreloaderPatches::jump_da(assembler, disassembler),
            PreloaderPatches::send_da(assembler, disassembler),
        ]
    }
}
