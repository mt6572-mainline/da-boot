use crate::{Assembler, Patch, PatchInformation, Result, err::Error};
use da_analyzer::{
    Analyzer,
    yaxpeax_arm::armv7::{Opcode, Operand},
};
use derive_ctor::ctor;

/// Disable hash check in the DA1
#[derive(ctor)]
pub struct UartPort<'a> {
    assembler: &'a Assembler,
    analyzer: &'a Analyzer,
}

/// Force uart0 port for DA logs
impl PatchInformation for UartPort<'_> {
    fn mode() -> crate::PatchMode {
        crate::PatchMode::Thumb2
    }

    fn ty() -> crate::PatchType {
        crate::PatchType::Instructions
    }
}

impl<'a> Patch<'a> for UartPort<'a> {
    fn new(assembler: &'a Assembler, analyzer: &'a Analyzer) -> Self {
        Self {
            assembler,
            analyzer,
        }
    }

    fn find(&self) -> Result<usize> {
        let (f, b_idx) = self
            .analyzer
            .find_string_ref("Output Log To Uart %d")
            .ok_or(Error::PatternNotFound)?;

        let target = f.blocks()[b_idx]
            .code()
            .iter()
            .find(|code| {
                let inst = code.instruction();
                if let Operand::Imm32(imm) = inst.operands[1]
                    && imm == 1
                    && inst.opcode == Opcode::MOV
                {
                    true
                } else {
                    false
                }
            })
            .ok_or(Error::PatternNotFound)?;
        Ok(target.offset())
    }

    fn replacement(&self) -> &'static str {
        "mov.w r0, #0"
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        let start = self.find()?;
        let replacement = self.assembler.thumb2(self.replacement())?;
        bytes[start..start + replacement.len()].clone_from_slice(&replacement);

        Ok(())
    }

    fn name() -> &'static str {
        "DA UART output"
    }
}
