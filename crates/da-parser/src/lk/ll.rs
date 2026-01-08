//! Low-level representation of the MediaTek LK structure
//!
//! This matches how LK is actually looks like.

use bincode::Decode;

use crate::{LLParser, lk::err::Error};

#[derive(Debug, Decode)]
#[repr(C)]
pub(crate) struct Header {
    magic: u32,
    size: u32,
    name: [u8; 32],
    pub load_address: u32,
    mode: u32,
    padding: [u8; 0x1d0],
}

impl LLParser for Header {
    type Error = Error;

    fn validate(&self) -> core::result::Result<(), Self::Error> {
        if self.magic != 0x58881688 {
            Err(Error::InvalidHeaderMagic(self.magic))
        } else if self.mode == 0 {
            Err(Error::InvalidHeaderMode(self.mode))
        } else {
            Ok(())
        }
    }
}
