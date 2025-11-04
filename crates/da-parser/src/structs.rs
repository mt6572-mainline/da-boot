use std::{borrow::Cow, ffi::CStr};

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
    count: u32,
}

impl Verify for DAHeader {
    fn verify(&self) -> Result<()> {
        if self.magic() != "MTK_DOWNLOAD_AGENT" {
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

impl DAHeader {
    fn magic(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.magic_string)
    }

    /// DA images count
    pub(crate) fn count(&self) -> u32 {
        self.count
    }
}

#[derive(Debug, Decode)]
#[repr(C)]
pub(crate) struct DAEntry {
    magic: u16,
    hw_code: u16,
    hw_subcode: u16,
    hw_version: u16,
    sw_version: u16,
    _unknown: [u16; 3],
    region_index: u16,
    region_count: u16,
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

impl DAEntry {
    pub(crate) fn hw_code(&self) -> u16 {
        self.hw_code
    }

    pub(crate) fn hw_subcode(&self) -> u16 {
        self.hw_subcode
    }

    pub(crate) fn hw_version(&self) -> u16 {
        self.hw_version
    }

    pub(crate) fn sw_version(&self) -> u16 {
        self.sw_version
    }

    pub(crate) fn region_count(&self) -> u16 {
        self.region_count
    }
}

#[derive(Decode)]
#[repr(C)]
pub(crate) struct DALoadRegion {
    /// Code offset
    pub start: u32,
    /// Code size
    pub len: u32,
    /// Code base address
    pub base: u32,
    offset: u32,
    /// Signature size
    pub sig_len: u32,
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

#[derive(Debug, Decode)]
#[repr(C)]
pub(crate) struct LKHeader {
    magic: u32,
    size: u32,
    name: [u8; 32],
    load_address: u32,
    mode: u32,
    _unused: [u8; 0x1d0],
}

impl Verify for LKHeader {
    fn verify(&self) -> Result<()> {
        if self.magic != 0x58881688 {
            return Err(Error::InvalidMagic);
        }

        if self.mode == 0 {
            return Err(Error::InvalidHeuristics);
        }

        Ok(())
    }
}

impl LKHeader {
    pub(crate) fn name(&self) -> Result<Cow<'_, str>> {
        Ok(CStr::from_bytes_until_nul(&self.name)?.to_string_lossy())
    }

    pub(crate) fn load_address(&self) -> u32 {
        self.load_address
    }
}
