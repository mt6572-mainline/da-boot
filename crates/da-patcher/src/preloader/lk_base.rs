use anyhow::{Context, Result};
use kaiko::Analyzer;

use crate::{Extract, extractor};

extractor!(LKBase);
impl Extract for LKBase<'_> {
    type Value = u32;

    fn extract(&self) -> Result<Self::Value> {
        let f = self
            .analyzer
            .fn_by_str("UBOOT")
            .or_else(|| self.analyzer.fn_by_str("%s Second Bootloader Load Failed"))
            .context("preloader main fn not found")?;
        let (base_address_block, code_va) = f
            .blocks()
            .find_map(|b| {
                let va = b.data_refs().find_map(|(code, ref_va)| {
                    (self
                        .analyzer
                        .read_cstr(ref_va)
                        .is_some_and(|s| s == "UBOOT" || s == "lk"))
                    .then_some(code.va())
                })?;
                Some((b, va))
            })
            .context("no block with parition string")?;

        // load method takes r3 as addr
        let lk_base = base_address_block
            .regs()
            // use code_va before fn call
            .try_get_imm(code_va, 3)
            .context("r3 state is unknown")?;

        Ok(lk_base)
    }
}
