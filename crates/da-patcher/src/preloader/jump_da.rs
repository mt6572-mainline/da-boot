use capstone::Instructions;

use crate::{
    Assembler, Disassembler, Patch, PatchCode, PatchInformation, Result, err::Error, slice::replace,
};
use derive_ctor::ctor;

/// Disable hardcoded value in the `jump_da` command
#[derive(ctor)]
pub struct JumpDA<'a> {
    assembler: &'a Assembler,
    disassembler: &'a Disassembler<'a>,
}

impl PatchInformation for JumpDA<'_> {
    fn mode() -> crate::PatchMode {
        crate::PatchMode::Thumb2
    }

    fn ty() -> crate::PatchType {
        crate::PatchType::Fuzzy
    }
}

impl PatchCode for JumpDA<'_> {
    fn assembler(&self) -> &Assembler {
        self.assembler
    }

    fn disassembler(&self) -> &Disassembler<'_> {
        self.disassembler
    }
}

impl Patch for JumpDA<'_> {
    fn pattern(&self) -> &'static str {
        "add r?, pc;\
         add r?, pc;\
         bl #?;\
         ldr r?, [pc, #?];\
         ldr r?, [pc, #?];\
         ? r1, #?;\
         add r?, pc;\
         add r?, pc"
    }

    fn offset(&self, bytes: &[u8]) -> Result<usize> {
        self.search(bytes).map(|o| o.end() + 4) // bl assert
    }

    fn replacement(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let offset = self.offset(bytes)?;
        let end = offset + 2 + (9 * 2) + (2 * 4); // ldr + 9x 16bit + mov.w + stm.w
        let bytes = &bytes[offset..=end];
        let instr = &self.disassembler.thumb2(bytes)?;
        let nop_count = Self::nop_count(instr)?;
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
        self.assembler.thumb2(&pattern)
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        replace(bytes, self.offset(bytes)?, &self.replacement(bytes)?);
        Ok(())
    }

    fn on_success(&self) -> &'static str {
        "jump_da is patched"
    }

    fn on_failure(&self) -> &'static str {
        "jump_da is not patched"
    }
}

impl JumpDA<'_> {
    /// Calculate NOP instruction count required for the patch
    fn nop_count(instr: &Instructions<'_>) -> Result<usize> {
        for (n, i) in instr.iter().enumerate() {
            if i.mnemonic().ok_or(Error::MnemonicNotAvailable)? == "stm.w" {
                return Ok(n
                    + (2 * instr
                        .iter()
                        // in jump_da all 32-bit instructions have dot
                        .filter(|i| i.mnemonic().is_some_and(|m| m.contains('.')))
                        .count()));
            }
        }

        Err(Error::PatternNotFound)
    }
}
