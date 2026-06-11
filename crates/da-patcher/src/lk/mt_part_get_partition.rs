use anyhow::{Context, Result};
use kaiko::Analyzer;

use crate::{Extract, extractor};

extractor!(MtPartGetPartition);
impl Extract for MtPartGetPartition<'_> {
    type Value = u32;

    fn extract(&self) -> Result<Self::Value> {
        let f = self
            .analyzer
            .fn_by_str("mt_part_get_partition")
            .context("string not found")?;

        Ok(f.start_va())
    }
}
