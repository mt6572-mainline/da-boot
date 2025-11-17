use crate::{Assembler, Disassembler, Patch, PatchCode, PatchInformation, Result, slice::replace};
use derive_ctor::ctor;

/// Disable Download Agent Authentication
#[derive(ctor)]
pub struct DAA<'a> {
    assembler: &'a Assembler,
    disassembler: &'a Disassembler<'a>,
}

impl PatchInformation for DAA<'_> {
    fn mode() -> crate::PatchMode {
        crate::PatchMode::Arm
    }

    fn ty() -> crate::PatchType {
        crate::PatchType::Instructions
    }
}

impl PatchCode for DAA<'_> {
    fn assembler(&self) -> &Assembler {
        self.assembler
    }

    fn disassembler(&self) -> &Disassembler<'_> {
        self.disassembler
    }
}

impl Patch for DAA<'_> {
    fn pattern(&self) -> &'static str {
        "ldr r3, [r3]; \
         ldr r2, [r3]; \
         cmp r2, #0x11"
    }

    fn offset(&self, bytes: &[u8]) -> Result<usize> {
        self.search(bytes).map(|o| o.start() - (3 * 4))
    }

    fn replacement(&self, _bytes: &[u8]) -> Result<Vec<u8>> {
        self.assembler.arm("movs r0, #0; bx lr")
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        replace(bytes, self.offset(bytes)?, &self.replacement(bytes)?);
        Ok(())
    }

    fn on_success(&self) -> &'static str {
        "DAA to be disabled"
    }

    fn on_failure(&self) -> &'static str {
        "DAA is not patched"
    }
}
