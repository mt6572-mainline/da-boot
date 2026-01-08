//! High-level representation of the MediaTek LK structure
//!
//! Intended for end use.
use std::fmt::Display;

use getset::Getters;

use crate::{HLParser, LLParser, lk::ll};

#[derive(Debug, Getters)]
pub struct LK {
    /// Load address
    #[getset(get = "pub")]
    load_address: u32,

    /// Executable code
    #[getset(get = "pub")]
    code: Vec<u8>,
}

impl HLParser<ll::Header> for LK {
    fn parse(data: &[u8], position: usize, ll: ll::Header) -> crate::Result<Self> {
        ll.validate()?;
        Ok(Self {
            load_address: ll.load_address,
            code: data[position..].to_vec(),
        })
    }
}

impl Display for LK {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Load address: {:#X}{}",
            self.load_address,
            if self.is_dummy_load_address() {
                " (dummy)"
            } else {
                ""
            }
        )?;
        write!(f, "Code: {} bytes", self.code.len())
    }
}

impl LK {
    /// Determines if the LK load address is a dummy value
    #[must_use]
    pub fn is_dummy_load_address(&self) -> bool {
        self.load_address == u32::MAX
    }
}
