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
