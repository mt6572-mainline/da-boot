use anyhow::{Context, Result};
use kaiko::{
    Analyzer,
    yaxpeax_arm::armv7::{Opcode, Operand},
};

use crate::{Extract, extractor};

extractor!(MtPartGenericRead);
impl Extract for MtPartGenericRead<'_> {
    type Value = u32;

    fn extract(&self) -> Result<Self::Value> {
        let f = self
            .analyzer
            .fn_by_str("[mt_part_register_device]\n")
            .context("string not found")?;
        let block = f
            .blocks()
            // block must NOT have POP
            .filter(|b| !b.code().any(|c| c.instruction().opcode.is_pop()))
            .find(|b| {
                b.code().count() < 10
                    && b.code().any(|c| {
                        if let Operand::RegDerefPreindexOffset(_, offset, _, _) =
                            c.instruction().operands[1]
                            && c.instruction().opcode == Opcode::STR
                        {
                            // read = 0x10
                            offset == 0x10
                        } else {
                            false
                        }
                    })
            })
            .context("no block with read fn")?;

        let str = block
            .code()
            .find(|c| c.instruction().opcode.is_str())
            .context("STR must exist")?;
        let Operand::Reg(r) = str.instruction().operands[0] else {
            unreachable!("operand must be a reg");
        };

        let va = block
            .regs()
            .try_get_imm(str.va(), r.number())
            .context("Failed to get imm")?;

        Ok(va)
    }
}
