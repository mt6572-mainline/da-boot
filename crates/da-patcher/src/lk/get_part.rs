use anyhow::{Context, Result};
use kaiko::Analyzer;

use crate::{Extract, extractor};

extractor!(GetPart);
impl Extract for GetPart<'_> {
    type Value = u32;

    fn extract(&self) -> Result<Self::Value> {
        let f = self
            .analyzer
            .fn_by_str("get_part")
            .context("string not found")?;

        Ok(f.start_va())
    }
}
