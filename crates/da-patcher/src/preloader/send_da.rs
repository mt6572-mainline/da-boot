use crate::{
    Assembler, Disassembler, Patch, PatchCode, PatchInformation, Result, err::Error, slice::replace,
};
use derive_ctor::ctor;

/// Disable hardcoded value in the `send_da` command
#[derive(ctor)]
pub struct SendDA<'a> {
    assembler: &'a Assembler,
    disassembler: &'a Disassembler<'a>,
}

impl PatchInformation for SendDA<'_> {
    fn mode() -> crate::PatchMode {
        crate::PatchMode::Thumb2
    }

    fn ty() -> crate::PatchType {
        crate::PatchType::Fuzzy
    }
}

impl PatchCode for SendDA<'_> {
    fn assembler(&self) -> &Assembler {
        self.assembler
    }

    fn disassembler(&self) -> &Disassembler<'_> {
        self.disassembler
    }
}

impl Patch for SendDA<'_> {
    fn pattern(&self) -> &'static str {
        "and r1, r3, #1;\
         ? r?, r3, #1;\
         mov r3, r0"
    }

    fn offset(&self, bytes: &[u8]) -> Result<usize> {
        let o = self.search(bytes)?;
        self.str_offset(bytes, o.start() - (8 * 2))
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

    fn on_success(&self) -> &'static str {
        "send_da is patched"
    }

    fn on_failure(&self) -> &'static str {
        "send_da is not patched"
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
