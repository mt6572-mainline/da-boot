use crate::{Assembler, Disassembler, Patch, PatchMessage, Result, err::Error, replace, search};

/// Disable hardcoded value in the `send_da` command
pub struct SendDA<'a> {
    assembler: &'a Assembler,
    disassembler: &'a Disassembler<'a>,
}

impl PatchMessage for SendDA<'_> {
    fn on_success() -> &'static str {
        "send_da hardcoded address is patched"
    }

    fn on_failure() -> &'static str {
        "send_da is not patched"
    }
}

impl<'a> Patch<'a> for SendDA<'a> {
    fn new(assembler: &'a Assembler, disassembler: &'a Disassembler) -> Self {
        Self {
            assembler,
            disassembler,
        }
    }

    fn pattern(&self) -> Result<Vec<u8>> {
        self.assembler.thumb2(
            "and.w r1, r3, #1; \
            lsrs r6, r3, #1; \
            mov r3, r0",
        )
    }

    fn offset(&self, bytes: &[u8]) -> Result<usize> {
        let o = search(bytes, &self.pattern()?).ok_or(Error::PatternNotFound)?;
        self.str_offset(bytes, o - (8 * 2))
    }

    fn replacement(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let offset = self.offset(bytes)?;
        let bytes = &bytes[offset..offset + 2];
        let instr = &self.disassembler.thumb2(bytes)?[0];
        self.assembler.thumb2(&format!(
            "ldr {}",
            instr.op_str().ok_or(Error::InstrOpNotAvailable)?
        ))
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        replace(bytes, self.offset(bytes)?, &self.replacement(bytes)?);
        Ok(())
    }
}

impl SendDA<'_> {
    /// Get `str r0, #[sp + offset]` instruction offset
    fn str_offset(&self, bytes: &[u8], position: usize) -> Result<usize> {
        let disasm = self.disassembler.thumb2(&bytes[position..position + 4])?;
        let (guessed_str, guessed_ldr) = (&disasm[0], &disasm[1]);
        if guessed_str.mnemonic().ok_or(Error::MnemonicNotAvailable)? == "ldr"
            && guessed_ldr.mnemonic().ok_or(Error::MnemonicNotAvailable)? == "movs"
        {
            Ok(position - 2)
        } else {
            Ok(position)
        }
    }
}
