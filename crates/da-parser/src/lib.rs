use std::fmt::Display;

use crate::{
    err::Error,
    structs::{DAEntry, DAHeader, DALoadRegion, LKHeader, Verify},
};

pub mod err;
mod structs;

pub type Result<T> = core::result::Result<T, Error>;

pub struct DA {
    pub hw_code: u16,
    hw_subcode: u16,
    hw_version: u16,
    sw_version: u16,

    pub regions: Vec<DARegion>,
}

impl Display for DA {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HW code: {:#x}\n", self.hw_code)?;
        write!(f, "HW subcode: {:#x}\n", self.hw_subcode)?;
        write!(f, "HW version: {:#x}\n", self.hw_version)?;
        write!(f, "SW version: {:#x}\n", self.sw_version)?;
        write!(f, "Regions:\n\t")?;
        for region in &self.regions {
            write!(f, "{}", region.to_string().replace("\n", "\n\t"))?;
        }

        Ok(())
    }
}

impl DA {
    pub(crate) fn from_raw(raw: DAEntry, regions: Vec<DARegion>) -> Self {
        DA {
            hw_code: raw.hw_code(),
            hw_subcode: raw.hw_subcode(),
            hw_version: raw.hw_version(),
            sw_version: raw.sw_version(),
            regions,
        }
    }
}

pub struct DARegion {
    pub base: u32,
    pub code: Vec<u8>,
    pub is_signed: bool,
}

impl DARegion {
    pub(crate) fn from_raw(raw: DALoadRegion, data: &[u8]) -> Self {
        Self {
            base: raw.base,
            code: data[raw.start as usize..(raw.start + raw.len) as usize].to_vec(),
            is_signed: raw.sig_len != 0,
        }
    }
}

impl Display for DARegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Base address: {:#x}\n", self.base)?;
        write!(f, "Code length: {:#x}\n", self.code.len())?;
        write!(f, "Signed: {}\n", if self.is_signed { "yes" } else { "no" })
    }
}

pub struct LK {
    partition_name: String,
    is_load_address_dummy: bool,
    pub code: Vec<u8>,
}

impl Display for LK {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Name: {}", self.partition_name)?;
        if self.is_load_address_dummy {
            write!(f, "Code load address is dummy")?;
        }

        Ok(())
    }
}

impl LK {
    pub(crate) fn try_from_raw(raw: LKHeader, data: &[u8]) -> Result<Self> {
        Ok(Self {
            partition_name: raw.name()?.into_owned(),
            is_load_address_dummy: raw.load_address() == u32::MAX,
            code: data[0x200..].to_vec(),
        })
    }
}

pub fn parse_da(data: &[u8]) -> Result<Vec<DA>> {
    let config = bincode::config::standard()
        .with_little_endian()
        .with_fixed_int_encoding();
    let (da, bytes_read): (DAHeader, _) = bincode::decode_from_slice(data, config)?;
    da.verify()?;

    let mut vec = Vec::with_capacity(da.count() as usize);
    for i in 0..da.count() {
        let (da_entry, offset): (DAEntry, _) =
            bincode::decode_from_slice(&data[bytes_read + (i as usize * 0xdc)..], config)?;
        da_entry.verify()?;

        let mut regions = Vec::with_capacity(da_entry.region_count() as usize);
        for j in 0..da_entry.region_count() {
            let region: DALoadRegion = bincode::decode_from_slice(
                &data[bytes_read + (i as usize * 0xdc) + offset + (j as usize * 0x14)..],
                config,
            )?
            .0;
            region.verify()?;
            regions.push(DARegion::from_raw(region, data));
        }

        vec.push(DA::from_raw(da_entry, regions))
    }

    Ok(vec)
}

pub fn parse_lk(data: &[u8]) -> Result<LK> {
    let config = bincode::config::standard()
        .with_little_endian()
        .with_fixed_int_encoding();
    let lk: LKHeader = bincode::decode_from_slice(data, config)?.0;
    lk.verify()?;

    LK::try_from_raw(lk, data)
}
