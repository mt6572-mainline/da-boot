#![feature(slice_pattern)]
#![feature(trait_alias)]
use std::marker::PhantomData;

use capstone::{Instructions, arch::BuildsCapstone};
use derive_ctor::ctor;
use hexpatch_keystone::Keystone;

use crate::err::Error;

pub mod err;
pub mod preloader;

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
    pub(crate) fn thumb2(&'a self, code: &[u8]) -> Result<Instructions<'a>> {
        Ok(self.thumb2.disasm_all(code, 0)?)
    }

    /// Disassemble `code` to arm instructions
    pub(crate) fn arm(&'a self, code: &[u8]) -> Result<Instructions<'a>> {
        Ok(self.arm.disasm_all(code, 0)?)
    }
}

pub(crate) trait PatchMessage {
    /// Message when the patch is applied
    fn on_success() -> &'static str
    where
        Self: Sized;
    /// Message when the patch is failed to apply
    fn on_failure() -> &'static str
    where
        Self: Sized;
}

pub(crate) trait Patch<'a> {
    /// Create new instance of the patch
    fn new(assembler: &'a Assembler, disassembler: &'a Disassembler) -> Self;
    /// Patch match pattern
    fn pattern(&self) -> Result<Vec<u8>>;
    /// Target offset to patch
    fn offset(&self, bytes: &[u8]) -> Result<usize>;
    /// Patch replacement code
    fn replacement(&self, bytes: &[u8]) -> Result<Vec<u8>>;
    /// Apply the patch to `bytes`
    fn patch(&self, bytes: &mut [u8]) -> Result<()>;
}

pub trait PatchCollection<'a, T: Sized> {
    /// Security-related patches
    fn security(assembler: &'a Assembler, disassembler: &'a Disassembler) -> Vec<T>;
    /// Hardcoded values-related patches
    fn hardcoded(assembler: &'a Assembler, disassembler: &'a Disassembler) -> Vec<T>;
}

/// Search in the `slice` for the `pattern`
///
/// Returns `None` if not found
#[inline]
fn search(slice: &[u8], pattern: &[u8]) -> Option<usize> {
    (0..slice.len()).find(|&i| slice[i..].starts_with(pattern))
}

/// Replace in `slice` starting with `at` position with `replacement`
#[inline]
fn replace(slice: &mut [u8], at: usize, replacement: &[u8]) {
    slice[at..at + replacement.len()].clone_from_slice(replacement);
}
