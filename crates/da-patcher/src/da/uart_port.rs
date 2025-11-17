use crate::{Assembler, Disassembler, Patch, PatchCode, PatchInformation, Result, slice::replace};
use derive_ctor::ctor;

/// Force uart0 port for DA logs
#[derive(ctor)]
pub struct UartPort<'a> {
    assembler: &'a Assembler,
    disassembler: &'a Disassembler<'a>,
}

impl PatchInformation for UartPort<'_> {
    fn mode() -> crate::PatchMode {
        crate::PatchMode::Thumb2
    }

    fn ty() -> crate::PatchType {
        crate::PatchType::Instructions
    }
}

impl PatchCode for UartPort<'_> {
    fn assembler(&self) -> &Assembler {
        self.assembler
    }

    fn disassembler(&self) -> &Disassembler<'_> {
        self.disassembler
    }
}

impl Patch for UartPort<'_> {
    fn pattern(&self) -> &'static str {
        "mov.w r2, #921600;\
         mov r1, r4"
    }

    fn offset(&self, bytes: &[u8]) -> Result<usize> {
        self.search(bytes).map(|o| o.start() - (2 + 4))
    }

    fn replacement(&self, _bytes: &[u8]) -> Result<Vec<u8>> {
        self.assembler.thumb2("mov.w r0, #0")
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        replace(bytes, self.offset(bytes)?, &self.replacement(bytes)?);
        Ok(())
    }

    fn on_success(&self) -> &'static str {
        "DA UART output is replaced"
    }

    fn on_failure(&self) -> &'static str {
        "DA UART output is not replaced"
    }
}
