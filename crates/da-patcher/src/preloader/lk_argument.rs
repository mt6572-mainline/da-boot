use anyhow::{Context, Result};
use kaiko::Analyzer;

use crate::{Extract, extractor};

extractor!(LKRanges);
impl Extract for LKRanges<'_> {
    type Value = (u32, u32);

    fn extract(&self) -> Result<Self::Value> {
        let f = self.analyzer.fn_by_str("UBOOT").context("fn not found")?;
        let (idx, base_address_block, code_va) = f
            .blocks()
            .enumerate()
            .find_map(|(i, b)| {
                let va = b.data_refs().find_map(|(code, ref_va)| {
                    (self.analyzer.read_cstr(ref_va) == Some("UBOOT")).then_some(code.va())
                })?;
                Some((i, b, va))
            })
            .context("no block with UBOOT string")?;

        // load method takes r3 as addr
        let lk_base = base_address_block
            .regs()
            // use code_va before fn call
            .try_get_imm(code_va, 3)
            .context("r3 state is unknown")?;

        // now take the next block
        let argument_block = f
            .blocks()
            .nth(idx + 1)
            .context("no block with LK argument addr")?;
        let end = argument_block
            .code()
            .last()
            .expect("BUG: code always must be > 0");
        // argument addr is r1
        let argument = argument_block
            .regs()
            .try_get_imm(end.va(), 1)
            .context("r1 state is unknown")?;

        Ok((lk_base, argument))
    }
}
