//! High-level representation of the MediaTek DA structure
//!
//! Intended for end use.
use std::{ffi::CStr, fmt::Display};

use getset::Getters;

use crate::{HLParser, LLParser, Result, da::ll};

#[derive(Debug, Getters)]
pub struct DA {
    /// Build ID
    #[getset(get = "pub", get_mut = "pub")]
    build_id: String,

    /// Entries per SoC
    #[getset(get = "pub", get_mut = "pub")]
    entries: Vec<Entry>,
}

impl HLParser<ll::Header> for DA {
    fn parse(data: &[u8], position: usize, ll: ll::Header) -> Result<Self> {
        ll.validate()?;
        Ok(Self {
            build_id: CStr::from_bytes_until_nul(&ll.build_id)?
                .to_string_lossy()
                .to_string(),
            entries: (0..ll.count as usize)
                .map(|i| {
                    let start = position + (i * 0xdc);
                    let ll = ll::Entry::parse(&data[start..])?;
                    Entry::parse(data, start + size_of::<ll::Entry>(), ll)
                })
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

impl Display for DA {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Build ID: {}", self.build_id)?;
        writeln!(f, "Entries:")?;
        for (i, entry) in self.entries.iter().enumerate() {
            writeln!(f, "Entry {}:", i + 1)?;
            for line in format!("{entry}").lines() {
                writeln!(f, "\t{line}")?;
            }
            if i != self.entries.len() - 1 {
                writeln!(f)?;
            }
        }

        Ok(())
    }
}

impl DA {
    /// Get DA entry by `hwcode`
    #[must_use]
    pub fn hwcode(&self, hwcode: u16) -> Option<&Entry> {
        self.entries.iter().find(|e| e.hw_code == hwcode)
    }

    /// Get DA entry by `hwcode`
    #[must_use]
    pub fn hwcode_mut(&mut self, hwcode: u16) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| e.hw_code == hwcode)
    }
}

#[derive(Debug, Getters)]
pub struct Entry {
    /// SoC hwcode
    #[getset(get = "pub", get_mut = "pub")]
    hw_code: u16,

    /// SoC hw subcode
    #[getset(get = "pub", get_mut = "pub")]
    hw_subcode: u16,

    /// SoC hw version
    #[getset(get = "pub", get_mut = "pub")]
    hw_version: u16,

    /// SoC sw version
    #[getset(get = "pub", get_mut = "pub")]
    sw_version: u16,

    /// Regions
    #[getset(get = "pub", get_mut = "pub")]
    regions: Vec<Region>,
}

impl HLParser<ll::Entry> for Entry {
    fn parse(data: &[u8], position: usize, ll: ll::Entry) -> Result<Self> {
        ll.validate()?;
        Ok(Self {
            hw_code: ll.hw_code,
            hw_subcode: ll.hw_subcode,
            hw_version: ll.hw_version,
            sw_version: ll.sw_version,
            regions: (0..ll.region_count as usize)
                .map(|i| {
                    let ll = ll::LoadRegion::parse(
                        &data[position + (i * size_of::<ll::LoadRegion>())..],
                    )?;
                    Region::parse(data, 0, ll)
                })
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

impl Display for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "HW code: {:#06X}", self.hw_code)?;
        writeln!(f, "HW subcode: {:#06X}", self.hw_subcode)?;
        writeln!(f, "HW version: {:#06X}", self.hw_version)?;
        writeln!(f, "SW version: {:#06X}", self.sw_version)?;
        writeln!(f, "Regions:")?;
        for (i, region) in self.regions.iter().enumerate() {
            match i {
                0 => writeln!(f, "\tHeader")?,
                1 => writeln!(f, "\tDA1")?,
                2 => writeln!(f, "\tDA2")?,
                _ => (),
            }
            for line in format!("{region}").lines() {
                writeln!(f, "\t{line}")?;
            }
            if i != self.regions.len() - 1 {
                writeln!(f)?;
            }
        }

        Ok(())
    }
}

impl Entry {
    /// DA1 region
    #[must_use]
    pub fn da1(&self) -> Option<&Region> {
        self.regions.get(1)
    }

    /// DA1 region
    #[must_use]
    pub fn da1_mut(&mut self) -> Option<&mut Region> {
        self.regions.get_mut(1)
    }

    /// DA2 region
    #[must_use]
    pub fn da2(&self) -> Option<&Region> {
        self.regions.get(2)
    }

    /// DA2 region
    #[must_use]
    pub fn da2_mut(&mut self) -> Option<&mut Region> {
        self.regions.get_mut(2)
    }
}

#[derive(Debug, Getters)]
pub struct Region {
    /// Executable code
    #[getset(get = "pub", get_mut = "pub")]
    code: Vec<u8>,

    /// Code signature
    #[getset(get = "pub", get_mut = "pub")]
    signature: Vec<u8>,

    /// Code base address
    #[getset(get = "pub", get_mut = "pub")]
    base: u32,
}

impl HLParser<ll::LoadRegion> for Region {
    fn parse(data: &[u8], _position: usize, ll: ll::LoadRegion) -> Result<Self> {
        ll.validate()?;
        let end = (ll.start + ll.len) as usize;

        Ok(Self {
            code: data[ll.start as usize..end - ll.sig_len as usize].to_vec(),
            signature: data[end - ll.sig_len as usize..end].to_vec(),
            base: ll.base,
        })
    }
}

impl Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Code: {} bytes", self.code.len())?;
        writeln!(f, "Signature: {} bytes", self.signature.len())?;
        write!(f, "Base address: {:#X}", self.base)
    }
}
