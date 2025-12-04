use crate::{
    Assembler, Disassembler, Patch, PatchCode, PatchInformation, Result, err::Error, extract_imm,
    slice::replace,
};
use derive_ctor::ctor;

/// DA boot argument
///
/// Overwritten with LK boot argument address
#[derive(ctor)]
pub struct DABootArgument<'a> {
    assembler: &'a Assembler,
    disassembler: &'a Disassembler<'a>,
}

impl PatchInformation for DABootArgument<'_> {
    fn mode() -> crate::PatchMode {
        crate::PatchMode::Thumb2
    }

    fn ty() -> crate::PatchType {
        crate::PatchType::Fuzzy
    }
}

impl PatchCode for DABootArgument<'_> {
    fn assembler(&self) -> &Assembler {
        self.assembler
    }

    fn disassembler(&self) -> &Disassembler<'_> {
        self.disassembler
    }
}

impl Patch for DABootArgument<'_> {
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
        let offset = self.search(bytes).map(|o| o.end() + 4)?; // bl assert
        let disasm = self
            .disassembler
            .thumb2(&bytes[offset..=offset + (3 * 4)])?;
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

    fn on_success(&self) -> &'static str {
        "jump_da boot argument is patched"
    }

    fn on_failure(&self) -> &'static str {
        "jump_da boot argument is not patched"
    }
}

impl DABootArgument<'_> {
    /// Parse PC-relative offset to the data
    fn data_offset(&self, bytes: &[u8], offset: usize) -> Result<usize> {
        let instr = &self.disassembler.thumb2(&bytes[offset..offset + 4])?[0];
        extract_imm(instr.op_str().ok_or(Error::InstrOpNotAvailable)?).map(|o| o + offset)
    }
}
