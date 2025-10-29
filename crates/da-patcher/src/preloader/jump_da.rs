use capstone::Instructions;

use crate::{Assembler, Disassembler, Patch, PatchMessage, Result, err::Error, replace, search};

/// Disable hardcoded value in the jump_da command
pub struct JumpDA<'a> {
    assembler: &'a Assembler,
    disassembler: &'a Disassembler<'a>,
}

impl PatchMessage for JumpDA<'_> {
    fn on_success() -> &'static str {
        "jump_da hardcoded address is patched"
    }

    fn on_failure() -> &'static str {
        "jump_da is not patched"
    }
}

impl<'a> Patch<'a> for JumpDA<'a> {
    fn new(assembler: &'a Assembler, disassembler: &'a Disassembler) -> Self {
        Self {
            assembler,
            disassembler,
        }
    }

    fn pattern(&self) -> Result<Vec<u8>> {
        self.assembler.thumb2(
            "ite ne; \
            movne r6, r3; \
            moveq r6, #0",
        )
    }

    fn offset(&self, bytes: &[u8]) -> Result<usize> {
        search(bytes, &self.pattern()?)
            .map(|o| o + (20 * 2) + 4) // bl assert
            .ok_or(Error::PatternNotFound)
    }

    fn replacement(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let offset = self.offset(bytes)?;
        let end = offset + 2 + (9 * 2) + (2 * 4); // ldr + 9x 16bit + mov.w + stm.w
        let bytes = &bytes[offset..=end];
        let instr = &self.disassembler.thumb2(bytes)?;
        let nop_count = self.nop_count(instr)?;
        let pattern = if instr
            .iter()
            .any(|i| i.mnemonic().is_some_and(|m| m == "mov.w"))
        {
            format!(
                "{} {}; {}", // da boot argument ldr; nops
                instr[0].mnemonic().ok_or(Error::MnemonicNotAvailable)?,
                instr[0].op_str().ok_or(Error::InstrOpNotAvailable)?,
                "nop;".repeat(nop_count - 1)
            )
        } else {
            format!(
                "nop; nop; {} {}; {}", // ldr magic; movs; da boot argument ldr; nops
                instr[2].mnemonic().ok_or(Error::MnemonicNotAvailable)?,
                instr[2].op_str().ok_or(Error::InstrOpNotAvailable)?,
                "nop;".repeat(nop_count - 3)
            )
        };
        self.assembler.thumb2(pattern)
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        replace(bytes, self.offset(bytes)?, &self.replacement(bytes)?);
        Ok(())
    }
}

impl JumpDA<'_> {
    /// Calculate NOP instruction count required for the patch
    fn nop_count(&self, instr: &Instructions<'_>) -> Result<usize> {
        for (n, i) in instr.iter().enumerate() {
            if i.mnemonic().ok_or(Error::MnemonicNotAvailable)? == "stm.w" {
                return Ok(n
                    + (2 * instr
                        .iter()
                        // in jump_da all 32-bit instructions have dot
                        .filter(|i| i.mnemonic().is_some_and(|m| m.contains(".")))
                        .count()));
            }
        }

        Err(Error::PatternNotFound)
    }
}
