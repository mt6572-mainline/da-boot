use crate::{Assembler, Disassembler, Patch, PatchMessage, Result, err::Error, replace, search};

/// Force uart0 port for DA logs
pub struct UartPort<'a> {
    assembler: &'a Assembler,
    _disassembler: &'a Disassembler<'a>,
}

impl PatchMessage for UartPort<'_> {
    fn on_success() -> &'static str {
        "Replaced uart1 with uart0"
    }

    fn on_failure() -> &'static str {
        "Failed to replace uart1"
    }
}

impl<'a> Patch<'a> for UartPort<'a> {
    fn new(assembler: &'a Assembler, _disassembler: &'a Disassembler) -> Self {
        Self {
            assembler,
            _disassembler,
        }
    }

    fn pattern(&self) -> Result<Vec<u8>> {
        self.assembler.thumb2(
            "mov.w r2, #921600;\
            mov r1, r4",
        )
    }

    fn offset(&self, bytes: &[u8]) -> Result<usize> {
        search(bytes, &self.pattern()?)
            .map(|o| o - (2 + 4))
            .ok_or(Error::PatternNotFound)
    }

    fn replacement(&self, _bytes: &[u8]) -> Result<Vec<u8>> {
        self.assembler.thumb2("mov.w r0, #0")
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        replace(bytes, self.offset(bytes)?, &self.replacement(bytes)?);
        Ok(())
    }
}
