use crate::{Assembler, Disassembler, Patch, PatchMessage, Result, err::Error, replace, search};

/// Disable hash check in the DA1
pub struct Hash<'a> {
    assembler: &'a Assembler,
    _disassembler: &'a Disassembler<'a>,
}

impl PatchMessage for Hash<'_> {
    fn on_success() -> &'static str {
        "Hash check is patched"
    }

    fn on_failure() -> &'static str {
        "Hash check is not patched"
    }
}

impl<'a> Patch<'a> for Hash<'a> {
    fn new(assembler: &'a Assembler, _disassembler: &'a Disassembler) -> Self {
        Self {
            assembler,
            _disassembler,
        }
    }

    fn pattern(&self) -> Result<Vec<u8>> {
        self.assembler.thumb2(
            "mov r2, sp;\
            sub.w r1, r9, #0x100;\
            mov r0, r5",
        )
    }

    fn offset(&self, bytes: &[u8]) -> Result<usize> {
        search(bytes, &self.pattern()?)
            .map(|o| o + (2 * 4) + (7 * 2))
            .ok_or(Error::PatternNotFound)
    }

    fn replacement(&self, _bytes: &[u8]) -> Result<Vec<u8>> {
        self.assembler.thumb2("cmp r1, r1")
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        replace(bytes, self.offset(bytes)?, &self.replacement(bytes)?);
        Ok(())
    }
}
