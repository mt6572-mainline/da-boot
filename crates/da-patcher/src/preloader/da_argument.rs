use regex::Regex;

use crate::{Assembler, Disassembler, Patch, PatchMessage, Result, err::Error, replace, search};

/// DA boot argument
///
/// Overwritten with LK boot argument address
pub struct DABootArgument<'a> {
    assembler: &'a Assembler,
    disassembler: &'a Disassembler<'a>,
}

impl PatchMessage for DABootArgument<'_> {
    fn on_success() -> &'static str {
        "jump_da boot argument hardcoded address is patched"
    }

    fn on_failure() -> &'static str {
        "jump_da boot argument address is not patched"
    }
}

impl<'a> Patch<'a> for DABootArgument<'a> {
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
        let offset = search(bytes, &self.pattern()?)
            .map(|o| o + (20 * 2) + 4) // bl assert
            .ok_or(Error::PatternNotFound)?;
        let disasm = self
            .disassembler
            .thumb2(&bytes[offset..=offset + (2 * 4)])?;
        Ok(
            self.data_offset(
                bytes,
                if disasm[3].mnemonic().is_some_and(|m| m == "movs") {
                    offset
                } else {
                    offset + (2 * 2)
                },
            )? + 2, // + 2 for ldr pc
        )
    }

    fn replacement(&self, _bytes: &[u8]) -> Result<Vec<u8>> {
        Ok(0x800d0000_u32.to_le_bytes().to_vec())
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        replace(bytes, self.offset(bytes)?, &self.replacement(bytes)?);
        Ok(())
    }
}

impl DABootArgument<'_> {
    /// Parse PC-relative offset to the data
    fn data_offset(&self, bytes: &[u8], offset: usize) -> Result<usize> {
        let instr = &self.disassembler.thumb2(&bytes[offset..offset + 2])?[0];
        let regex = Regex::new("#0x([0-9A-Fa-f]+)")?;
        Ok(usize::from_str_radix(
            regex
                .find(instr.op_str().ok_or(Error::InstrOpNotAvailable)?)
                .ok_or(Error::PatternNotFound)?
                .as_str()
                .trim_start_matches("#0x"),
            16,
        )? + offset)
    }
}
