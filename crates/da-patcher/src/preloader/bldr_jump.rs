use anyhow::{Context, Result};
use kaiko::{Analyzer, yaxpeax_arm::armv7::Opcode};

use crate::{Extract, extractor};

extractor!(BldrJump);
impl Extract for BldrJump<'_> {
    type Value = (u32, u32);

    fn extract(&self) -> Result<Self::Value> {
        let f = self
            .analyzer
            .fn_by_str("%s usbdl_jump_da: %x\n")
            .context("string not found")?;

        // we need a block with at least 2 literal loads and 4 BLX calls
        let block = f
            .blocks()
            .find(|b| {
                b.code()
                    .filter(|c| c.instruction().opcode == Opcode::LDR)
                    .count()
                    >= 2
                    && b.code()
                        .filter(|c| c.instruction().opcode == Opcode::BLX)
                        .count()
                        >= 4
            })
            .context("bldr_jump block not found: need at least 2 literal loads and 4 BLX calls")?;

        let (_, bldr_jump) = block
            .fn_calls()
            .last()
            .context("calls in the bldr_jump block must exist")?;

        // what a hack.
        let da_addr = block
            .data_refs()
            .find_map(|(_, v)| ((v & !0xfff) == v).then_some(v))
            .context("DA addr not found")?;
        Ok((bldr_jump, da_addr))
    }
}
