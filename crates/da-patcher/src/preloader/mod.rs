use crate::{PatchCollection, preloader::sec_region_check::SecRegionCheck};
use da_boot_macros::PatchEnum;
use enum_dispatch::enum_dispatch;

pub mod sec_region_check;

/// Preloader patches
#[enum_dispatch(Patch)]
#[derive(PatchEnum)]
pub enum PreloaderPatches<'a> {
    /// `sec_region_check` function patch
    SecRegionCheck(SecRegionCheck<'a>),
}

impl PreloaderPatches<'_> {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::SecRegionCheck(_) => "sec_region_check",
        }
    }
}

pub struct Preloader;
impl<'a> PatchCollection<'a, PreloaderPatches<'a>> for Preloader {
    fn security(
        assembler: &'a crate::Assembler,
        disassembler: &'a crate::Disassembler,
    ) -> Vec<PreloaderPatches<'a>> {
        vec![PreloaderPatches::sec_region_check(assembler, disassembler)]
    }

    fn hardcoded(
        _assembler: &'a crate::Assembler,
        _disassembler: &'a crate::Disassembler,
    ) -> Vec<PreloaderPatches<'a>> {
        vec![]
    }
}
