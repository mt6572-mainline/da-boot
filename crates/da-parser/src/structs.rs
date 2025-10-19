use bincode::Decode;

use crate::{Result, err::Error};

pub(crate) trait Verify {
    fn verify(&self) -> Result<()>;
}

#[derive(Debug, Decode)]
#[repr(C)]
pub(crate) struct DAHeader {
    magic_string: [u8; 18],
    padding: [u8; 14],
    build_id: [u8; 64],
    /// Always 0x4 (?)
    unknown: u32,
    /// 0x55663388 if encrypted
    magic: u32,
    pub count: u32,
}

impl Verify for DAHeader {
    fn verify(&self) -> Result<()> {
        if String::from_utf8_lossy(&self.magic_string) != "MTK_DOWNLOAD_AGENT" {
            return Err(Error::InvalidMagic);
        }

        if self.padding.iter().any(|e| *e != 0) {
            return Err(Error::InvalidHeuristics);
        }

        if self.unknown != 0x4 {
            return Err(Error::InvalidHeuristics);
        }

        if self.magic != 0x22668899 {
            return Err(Error::InvalidMagic);
        }

        Ok(())
    }
}

#[derive(Debug, Decode)]
#[repr(C)]
pub(crate) struct DAEntry {
    magic: u16,
    pub hw_code: u16,
    pub hw_subcode: u16,
    pub hw_version: u16,
    pub sw_version: u16,
    _unknown: [u16; 3],
    region_index: u16,
    pub region_count: u16,
}

impl Verify for DAEntry {
    fn verify(&self) -> Result<()> {
        if self.magic != 0xDADA {
            return Err(Error::InvalidMagic);
        }

        if self.region_count == 0 {
            return Err(Error::InvalidRegionCount);
        }

        Ok(())
    }
}

#[derive(Decode)]
#[repr(C)]
pub(crate) struct DALoadRegion {
    pub start: u32,
    pub len: u32,
    pub base: u32,
    offset: u32,
    sig_len: u32,
}

impl Verify for DALoadRegion {
    fn verify(&self) -> Result<()> {
        if self.start < 0x100 {
            return Err(Error::InvalidRegionStart);
        }

        if self.len < 0x100 {
            return Err(Error::InvalidCodeSize);
        }

        Ok(())
    }
}
