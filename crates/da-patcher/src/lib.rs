pub mod err;
pub mod lk;
pub mod preloader;

use anyhow::Result;

/// Extract value from the binary
pub trait Extract {
    /// Extraction output
    type Value;

    fn extract(&self) -> Result<Self::Value>;
}

macro_rules! extractor {
    ($name:ident) => {
        pub struct $name<'a> {
            analyzer: &'a Analyzer,
        }

        impl<'a> $name<'a> {
            pub fn new(analyzer: &'a Analyzer) -> Self {
                Self { analyzer }
            }
        }
    };
}
pub(crate) use extractor;

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{
        lk::{mt_part_generic_read::MtPartGenericRead, mt_part_get_partition::MtPartGetPartition},
        preloader::{bldr_jump::BldrJump, lk_argument::LKRanges, usb_ptr::PreloaderDLULPtr},
    };

    use super::*;
    use kaiko::Analyzer;

    #[test]
    fn preloader() {
        let data = fs::read("test-pl.bin").unwrap();
        let analyzer = Analyzer::try_new(data.into(), 0x2007500, 0, kaiko::cpu_mode::CpuMode::Arm)
            .expect("analyzer must init");

        PreloaderDLULPtr::new(&analyzer)
            .extract()
            .expect("error on extracting pointers");

        LKRanges::new(&analyzer)
            .extract()
            .expect("error on extracting lk memory");

        BldrJump::new(&analyzer)
            .extract()
            .expect("error on extracting bldr_jump ptr");
    }

    #[test]
    fn lk() {
        let data = fs::read("lk-no-hdr-d101.bin").unwrap();
        let analyzer = Analyzer::try_new(data.into(), 0x80020000, 0, kaiko::cpu_mode::CpuMode::Arm)
            .expect("analyzer must init");

        MtPartGetPartition::new(&analyzer)
            .extract()
            .expect("error on extracting mt_part_get_partition");

        MtPartGenericRead::new(&analyzer)
            .extract()
            .expect("error on extracting mt_part_generic_read");
    }
}
