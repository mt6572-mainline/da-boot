use crate::{Assembler, Disassembler, Patch, PatchCode, PatchInformation, Result, slice::replace};
use derive_ctor::ctor;

/// Disable hash check in the DA1
#[derive(ctor)]
pub struct Hash<'a> {
    assembler: &'a Assembler,
    disassembler: &'a Disassembler<'a>,
}

impl PatchInformation for Hash<'_> {
    fn mode() -> crate::PatchMode {
        crate::PatchMode::Thumb2
    }

    fn ty() -> crate::PatchType {
        crate::PatchType::Instructions
    }
}

impl PatchCode for Hash<'_> {
    fn assembler(&self) -> &Assembler {
        self.assembler
    }

    fn disassembler(&self) -> &Disassembler<'_> {
        self.disassembler
    }
}

impl Patch for Hash<'_> {
    fn pattern(&self) -> &'static str {
        "mov r2, sp;\
         sub.w r1, r9, #0x100;\
         mov r0, r5"
    }

    fn offset(&self, bytes: &[u8]) -> Result<usize> {
        self.search(bytes).map(|o| o.end() + 4 + (5 * 2))
    }

    fn replacement(&self, _bytes: &[u8]) -> Result<Vec<u8>> {
        self.assembler.thumb2("cmp r1, r1")
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        replace(bytes, self.offset(bytes)?, &self.replacement(bytes)?);
        Ok(())
    }

    fn on_success(&self) -> &'static str {
        "DA1 hash check is patched"
    }

    fn on_failure(&self) -> &'static str {
        "DA1 hash check is not patched"
    }
}
