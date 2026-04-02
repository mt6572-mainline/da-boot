use crate::{Assembler, Patch, PatchInformation, Result, err::Error};
use da_analyzer::{Analyzer, yaxpeax_arm::armv7::Opcode};
use derive_ctor::ctor;

/// Disable assert when reading SRAM or DRAM contents with the read32 command
#[derive(ctor)]
pub struct HwCheckBattery<'a> {
    assembler: &'a Assembler,
    analyzer: &'a Analyzer,
}

impl PatchInformation for HwCheckBattery<'_> {
    fn mode() -> crate::PatchMode {
        crate::PatchMode::Thumb2
    }

    fn ty() -> crate::PatchType {
        crate::PatchType::Instructions
    }
}

impl<'a> Patch<'a> for HwCheckBattery<'a> {
    fn new(assembler: &'a Assembler, analyzer: &'a Analyzer) -> Self {
        Self {
            assembler,
            analyzer,
        }
    }

    fn find(&self) -> Result<usize> {
        let (f, b_idx) = self
            .analyzer
            .find_string_ref("No Battery")
            .ok_or(Error::PatternNotFound)?;

        let target = f.blocks()[b_idx]
            .code()
            .iter()
            .find(|code| code.instruction().opcode == Opcode::CMP)
            .ok_or(Error::PatternNotFound)?;
        Ok(target.offset())
    }

    fn replacement(&self) -> &'static str {
        "cmp r0, r1"
    }

    fn patch(&self, bytes: &mut [u8]) -> Result<()> {
        let start = self.find()?;
        let replacement = self.assembler.thumb2(self.replacement())?;
        bytes[start..start + replacement.len()].clone_from_slice(&replacement);

        Ok(())
    }

    fn name() -> &'static str {
        "Preloader battery check"
    }
}
