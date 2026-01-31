//! High-level representation of the MediaTek LK structure
//!
//! Intended for end use.
use std::{borrow::Cow, fmt::Display};

use crate::{HLParser, LLParser, lk::ll};
use derive_ctor::ctor;
use getset::Getters;

#[derive(Debug, Getters, ctor)]
pub struct LK<'a> {
    /// Load address
    #[getset(get = "pub")]
    load_address: u32,

    /// Name
    #[getset(get = "pub")]
    name: String,

    /// Executable code
    #[getset(get = "pub")]
    code: Cow<'a, [u8]>,
}

impl<'a> HLParser<'a, ll::Header> for LK<'a> {
    fn parse(data: &'a [u8], position: usize, ll: ll::Header) -> crate::Result<Self> {
        ll.validate()?;
        Ok(Self {
            load_address: ll.load_address,
            name: String::from_utf8_lossy(&ll.name).into_owned(),
            code: Cow::Borrowed(&data[position..]),
        })
    }

    fn as_ll(&self) -> crate::Result<ll::Header> {
        ll::Header::try_new(
            self.code.len() as u32,
            &self.name,
            Some(self.load_address),
            0,
        )
    }
}

impl Display for LK<'_> {
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

impl LK<'_> {
    /// Determines if the LK load address is a dummy value
    #[must_use]
    pub fn is_dummy_load_address(&self) -> bool {
        self.load_address == u32::MAX
    }
}
