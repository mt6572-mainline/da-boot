use da_analyzer::Analyzer;
use derive_ctor::ctor;
use derive_more::IsVariant;
use hexpatch_keystone::Keystone;

use crate::err::Error;

pub mod da;
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

#[derive(IsVariant)]
pub enum PatchMode {
    Arm,
    Thumb2,
}

pub enum PatchType {
    Instructions,
    Fuzzy,
}

pub trait PatchInformation {
    /// CPU mode to target
    fn mode() -> PatchMode;
    /// Patch type
    fn ty() -> PatchType;
}

pub trait Patch<'a> {
    fn new(assembler: &'a Assembler, analyzer: &'a Analyzer) -> Self;

    /// Find required code offset to patch
    fn find(&self) -> Result<usize>;
    /// Replacement code
    fn replacement(&self) -> &'static str;
    /// Apply the patch to `bytes`
    fn patch(&self, bytes: &mut [u8]) -> Result<()>;

    fn name() -> &'static str;
}

pub fn oneshot<'a, T: Patch<'a>>(
    asm: &'a Assembler,
    analyzer: &'a Analyzer,
    bytes: &mut [u8],
) -> Result<()> {
    T::new(asm, analyzer).patch(bytes)
}
