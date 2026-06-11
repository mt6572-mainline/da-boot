use anyhow::{Context, Result};
use kaiko::{Analyzer, yaxpeax_arm::armv7::Operand};

use crate::{Extract, extractor};

extractor!(PreloaderDLULPtr);
impl Extract for PreloaderDLULPtr<'_> {
    type Value = (u32, u32);

    fn extract(&self) -> Result<Self::Value> {
        let block = self
            .analyzer
            .blocks_by_str("%s sync time %dms\n")
            // there's more than 1 block referencing this string, so we need to find
            // the correct one
            .find(|b| b.code().any(|c| c.instruction().opcode.is_ldm()))
            .context("no block with ldm instruction")?;
        let ldm = block
            .code()
            .find(|code| code.instruction().opcode.is_ldm())
            .expect("BUG: ldm must exist if the block was found");
        let Operand::RegWBack(r, _) = ldm.instruction().operands[0] else {
            unreachable!("BUG: operand must be writeback reg");
        };
        let array = block
            .regs()
            .try_get_imm(ldm.va(), r.number())
            .context("reg state is unknown")?;
        let ptr_ul = self
            .analyzer
            .read_u32(array)
            .with_context(|| format!("failed to read ptr dl from {array:#x}"))?;
        let ptr_dl = self
            .analyzer
            .read_u32(array + 4)
            .with_context(|| format!("failed to read ptr ul from {array:#x}"))?;

        Ok((ptr_dl, ptr_ul))
    }
}
