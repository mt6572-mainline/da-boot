use anyhow::{Context, Result};
use kaiko::Analyzer;

use crate::{Extract, extractor};

extractor!(BldrJump);
impl Extract for BldrJump<'_> {
    type Value = u32;

    fn extract(&self) -> Result<Self::Value> {
        const JUMP_DA: &str = "%s usbdl_jump_da: %x\n";

        let f = self
            .analyzer
            .fn_by_str(JUMP_DA)
            .context("string not found")?;
        let block = self
            .analyzer
            .block_by_str(JUMP_DA)
            .context("string not found")?;
        let idx = f
            .blocks()
            .position(|b| b == block)
            .expect("BUG: fn must contain the block");

        // bldr_jump block is the next one
        let next = f
            .blocks()
            .nth(idx + 1)
            .context("unexpected end of function")?;

        let (_, bldr_jump) = next
            .fn_calls()
            .last()
            .context("calls in the bldr_jump block must exist")?;

        Ok(bldr_jump)
    }
}
