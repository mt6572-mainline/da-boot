use crate::{Assembler, Disassembler, Patch, PatchMessage, Result, err::Error, replace, search};

/// Disable assert when reading SRAM or DRAM contents with the read32 command
pub struct SecRegionCheck<'a> {
    assembler: &'a Assembler,
    _disassembler: &'a Disassembler<'a>,
}

impl PatchMessage for SecRegionCheck<'_> {
    fn on_success() -> &'static str {
        "sec_region_check is patched"
    }

    fn on_failure() -> &'static str {
        "sec_region_check is not patched"
    }
}

impl<'a> Patch<'a> for SecRegionCheck<'a> {
    fn new(assembler: &'a Assembler, _disassembler: &'a Disassembler) -> Self {
        Self {
            assembler,
            _disassembler,
        }
    }

    fn pattern(&self) -> Result<Vec<u8>> {
        self.assembler.thumb2(
            "push {r0,r1,r2,r4,r5,lr}; \
            mov r4, r0; \
            mov r5, r1",
        )
    }

    fn offset(&self, bytes: &[u8]) -> Result<usize> {
        search(bytes, &self.pattern()?).ok_or(Error::PatternNotFound)
    }

    fn replacement(&self, _bytes: &[u8]) -> Result<Vec<u8>> {
        self.assembler.thumb2("movs r0, #0; bx lr")
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        replace(bytes, self.offset(bytes)?, &self.replacement(bytes)?);
        Ok(())
    }
}
