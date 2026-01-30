//! Low-level representation of the MediaTek LK structure
//!
//! This matches how LK is actually looks like.

use std::ffi::CString;

use bincode::{Decode, Encode};

use crate::{LLParser, Result, lk::err::Error};

const MAGIC: u32 = 0x58881688;

#[derive(Debug, Decode, Encode)]
#[repr(C)]
pub(crate) struct Header {
    magic: u32,
    size: u32,
    pub name: [u8; 32],
    pub load_address: u32,
    mode: u32,
    padding: [u8; 0x1d0],
}

impl LLParser for Header {
    type Error = Error;

    fn validate(&self) -> core::result::Result<(), Self::Error> {
        if self.magic != MAGIC {
            Err(Error::InvalidHeaderMagic(self.magic))
        } else if self.mode == 0 {
            Err(Error::InvalidHeaderMode(self.mode))
        } else {
            Ok(())
        }
    }
}

impl Header {
    pub fn try_new(size: u32, name: &str, load_address: Option<u32>, mode: u32) -> Result<Self> {
        Ok(Self {
            magic: MAGIC,
            size,
            name: CString::new(name)?.as_bytes_with_nul().try_into()?,
            load_address: load_address.unwrap_or(u32::MAX),
            mode,
            padding: [0; 0x1d0],
        })
    }
}
