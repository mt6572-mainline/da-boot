//! Low-level representation of the MediaTek DA structure
//!
//! This matches how DA is actually looks like.
use bincode::Decode;

use crate::{LLParser, da::err::Error};

#[derive(Debug, Decode)]
#[repr(C)]
pub(crate) struct Header {
    magic: [u8; 18],
    padding: [u8; 14],
    pub build_id: [u8; 64],
    unknown: u32,
    ty: u32,
    pub count: u32,
}

impl LLParser for Header {
    type Error = Error;

    fn validate(&self) -> core::result::Result<(), Self::Error> {
        if &self.magic != b"MTK_DOWNLOAD_AGENT" {
            Err(Error::InvalidHeaderMagic(self.magic))
        } else if self.padding.iter().any(|b| *b != 0) {
            Err(Error::InvalidHeaderHeuristics)
        } else if self.unknown != 0x4 {
            Err(Error::InvalidHeaderHeuristics)
        } else if self.ty != 0x22668899 {
            Err(Error::InvalidHeaderType(self.ty))
        } else {
            Ok(())
        }
    }
}
#[derive(Debug, Decode)]
#[repr(C)]
pub(crate) struct Entry {
    pub magic: u16,
    pub hw_code: u16,
    pub hw_subcode: u16,
    pub hw_version: u16,
    pub sw_version: u16,
    unknown: [u16; 3],
    region_index: u16,
    pub region_count: u16,
}

impl LLParser for Entry {
    type Error = Error;

    fn validate(&self) -> core::result::Result<(), Self::Error> {
        if self.magic != 0xDADA {
            Err(Error::InvalidEntryMagic(self.magic))
        } else if self.region_count == 0 {
            Err(Error::InvalidEntryHeuristics)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Decode)]
#[repr(C)]
pub(crate) struct LoadRegion {
    pub start: u32,
    pub len: u32,
    pub base: u32,
    pub offset: u32,
    pub sig_len: u32,
}

impl LLParser for LoadRegion {
    type Error = Error;

    fn validate(&self) -> core::result::Result<(), Self::Error> {
        if self.start < 0x100 {
            Err(Error::InvalidRegionStart(self.start))
        } else if self.len < 0x100 {
            Err(Error::InvalidRegionSize(self.len))
        } else if self.base == 0 {
            Err(Error::InvalidRegionSize(self.base))
        } else {
            Ok(())
        }
    }
}
