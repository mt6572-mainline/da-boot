use crate::{Assembler, Disassembler, Patch, PatchMessage, Result, err::Error, replace, search};

/// Disable Download Agent Authentication
pub struct DAA<'a> {
    assembler: &'a Assembler,
    _disassembler: &'a Disassembler<'a>,
}

impl PatchMessage for DAA<'_> {
    fn on_success() -> &'static str {
        "DAA to be disabled"
    }

    fn on_failure() -> &'static str {
        "Failed to disable DAA"
    }
}

impl<'a> Patch<'a> for DAA<'a> {
    fn new(assembler: &'a Assembler, _disassembler: &'a Disassembler) -> Self {
        Self {
            assembler,
            _disassembler,
        }
    }

    fn pattern(&self) -> Result<Vec<u8>> {
        self.assembler.arm(
            "ldr r3, [r3]; \
            ldr r2, [r3]; \
            cmp r2, #0x11",
        )
    }

    fn offset(&self, bytes: &[u8]) -> Result<usize> {
        search(bytes, &self.pattern()?)
            .map(|o| o - (3 * 4))
            .ok_or(Error::PatternNotFound)
    }

    fn replacement(&self, _bytes: &[u8]) -> Result<Vec<u8>> {
        self.assembler.arm("movs r0, #0; bx lr")
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        replace(bytes, self.offset(bytes)?, &self.replacement(bytes)?);
        Ok(())
    }
}
