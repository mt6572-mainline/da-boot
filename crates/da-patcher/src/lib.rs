use std::{marker::PhantomData, ops::RangeInclusive};

use capstone::{Instructions, arch::BuildsCapstone};
use derive_ctor::ctor;
use derive_more::IsVariant;
use enum_dispatch::enum_dispatch;
use hexpatch_keystone::Keystone;
use regex::Regex;

use crate::{
    da::{DAPatches, hash::Hash, uart_port::UartPort},
    err::Error,
    preloader::{PreloaderPatches, sec_region_check::SecRegionCheck},
    slice::{
        fuzzy::{fuzzy_search_thumb2, generic_reg_matcher},
        search,
    },
};

pub mod da;
pub mod err;
pub mod preloader;
pub mod slice;

pub type Result<T> = core::result::Result<T, Error>;

/// Keystone code assembler
#[derive(ctor)]
pub struct Assembler {
    arm: Keystone,
    thumb2: Keystone,
}

impl Assembler {
    pub fn try_new() -> Result<Self> {
        Ok(Self {
            arm: Keystone::new(hexpatch_keystone::Arch::ARM, hexpatch_keystone::Mode::ARM)?,
            thumb2: Keystone::new(hexpatch_keystone::Arch::ARM, hexpatch_keystone::Mode::THUMB)?,
        })
    }

    /// Assemble `code` to Thumb2 instructions
    pub(crate) fn thumb2<T: ToString + ?Sized>(&self, code: &T) -> Result<Vec<u8>> {
        Ok(self.thumb2.asm(code.to_string(), 0)?.bytes)
    }

    /// Assemble `code` to arm instructions
    pub(crate) fn arm<T: ToString + ?Sized>(&self, code: &T) -> Result<Vec<u8>> {
        Ok(self.arm.asm(code.to_string(), 0)?.bytes)
    }
}

/// Capstone code disassembler
#[derive(ctor)]
pub struct Disassembler<'a> {
    arm: capstone::Capstone,
    thumb2: capstone::Capstone,
    _phantom: PhantomData<&'a capstone::Capstone>,
}

impl<'a> Disassembler<'a> {
    pub fn try_new() -> Result<Self> {
        Ok(Self {
            arm: capstone::Capstone::new()
                .arm()
                .mode(capstone::arch::arm::ArchMode::Arm)
                .build()?,
            thumb2: capstone::Capstone::new()
                .arm()
                .mode(capstone::arch::arm::ArchMode::Thumb)
                .build()?,
            _phantom: PhantomData,
        })
    }

    /// Disassemble `code` to Thumb2 instructions
    pub fn thumb2(&'a self, code: &[u8]) -> Result<Instructions<'a>> {
        Ok(self.thumb2.disasm_all(code, 0)?)
    }

    /// Disassemble `code` to arm instructions
    pub fn arm(&'a self, code: &[u8]) -> Result<Instructions<'a>> {
        Ok(self.arm.disasm_all(code, 0)?)
    }

    pub fn thumb2_disasm_count(&'a self, code: &[u8], count: usize) -> Result<Instructions<'a>> {
        Ok(self.thumb2.disasm_count(code, 0, count)?)
    }

    pub fn arm_disasm_count(&'a self, code: &[u8], count: usize) -> Result<Instructions<'a>> {
        Ok(self.arm.disasm_count(code, 0, count)?)
    }
}

#[derive(IsVariant)]
pub enum PatchMode {
    Arm,
    Thumb2,
}

pub enum PatchType {
    Instructions,
    Fuzzy,
}

#[enum_dispatch]
pub trait PatchInformation {
    /// CPU mode to target
    fn mode() -> PatchMode;
    /// Patch type
    fn ty() -> PatchType;
}

pub(crate) trait PatchCode: PatchInformation + Patch {
    fn assembler(&self) -> &Assembler;
    fn disassembler(&self) -> &Disassembler<'_>;

    fn search(&self, slice: &[u8]) -> Result<RangeInclusive<usize>> {
        let pattern = self.pattern();

        match Self::ty() {
            PatchType::Instructions => {
                let pattern = if Self::mode().is_arm() {
                    self.assembler().arm(pattern)
                } else {
                    self.assembler().thumb2(pattern)
                }?;

                search(slice, &pattern)
                    .map(|start| start..=start + pattern.len())
                    .ok_or(Error::PatternNotFound)
            }
            PatchType::Fuzzy => {
                if Self::mode().is_arm() {
                    return Err(Error::Custom("ARM mode is not supported".into()));
                }

                fuzzy_search_thumb2(self.disassembler(), slice, pattern, generic_reg_matcher)
            }
        }
    }
}

#[enum_dispatch]
pub trait Patch {
    /// Patch match pattern
    fn pattern(&self) -> &'static str;
    /// Target offset to patch
    fn offset(&self, bytes: &[u8]) -> Result<usize>;
    /// Patch replacement code
    fn replacement(&self, bytes: &[u8]) -> Result<Vec<u8>>;
    /// Apply the patch to `bytes`
    fn patch(&self, bytes: &mut [u8]) -> Result<()>;

    fn on_success(&self) -> &'static str;
    fn on_failure(&self) -> &'static str;
}

pub trait PatchCollection<'a, T: Sized> {
    /// Security-related patches
    #[must_use]
    fn security(assembler: &'a Assembler, disassembler: &'a Disassembler) -> Vec<T>;
    /// Hardcoded values-related patches
    #[must_use]
    fn hardcoded(assembler: &'a Assembler, disassembler: &'a Disassembler) -> Vec<T>;

    /// Security and hardcoded patches
    #[must_use]
    fn all(assembler: &'a Assembler, disassembler: &'a Disassembler) -> Vec<T> {
        [
            Self::security(assembler, disassembler),
            Self::hardcoded(assembler, disassembler),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

pub fn extract_imm(s: &str) -> Result<usize> {
    let regex = Regex::new("#(0x)?([0-9A-Fa-f]+)")?;
    Ok(usize::from_str_radix(
        regex
            .find(s)
            .ok_or(Error::PatternNotFound)?
            .as_str()
            .trim_start_matches("#")
            .trim_start_matches("0x"),
        16,
    )?)
}
