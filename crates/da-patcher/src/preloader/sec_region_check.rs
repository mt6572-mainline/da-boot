use crate::{Assembler, Disassembler, Patch, PatchCode, PatchInformation, Result, slice::replace};
use derive_ctor::ctor;

/// Disable assert when reading SRAM or DRAM contents with the read32 command
#[derive(ctor)]
pub struct SecRegionCheck<'a> {
    assembler: &'a Assembler,
    disassembler: &'a Disassembler<'a>,
}

impl PatchInformation for SecRegionCheck<'_> {
    fn mode() -> crate::PatchMode {
        crate::PatchMode::Thumb2
    }

    fn ty() -> crate::PatchType {
        crate::PatchType::Instructions
    }
}

impl PatchCode for SecRegionCheck<'_> {
    fn assembler(&self) -> &Assembler {
        self.assembler
    }

    fn disassembler(&self) -> &Disassembler<'_> {
        self.disassembler
    }
}

impl Patch for SecRegionCheck<'_> {
    fn pattern(&self) -> &'static str {
        "push {r0,r1,r2,r4,r5,lr}; \
         mov r4, r0; \
         mov r5, r1"
    }

    fn offset(&self, bytes: &[u8]) -> Result<usize> {
        self.search(bytes).map(|o| *o.start())
    }

    fn replacement(&self, _bytes: &[u8]) -> Result<Vec<u8>> {
        self.assembler.thumb2("movs r0, #0; bx lr")
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        replace(bytes, self.offset(bytes)?, &self.replacement(bytes)?);
        Ok(())
    }

    fn on_success(&self) -> &'static str {
        "sec_region_check is patched"
    }

    fn on_failure(&self) -> &'static str {
        "sec_region_check is not patched"
    }
}
