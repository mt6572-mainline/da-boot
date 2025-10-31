use std::fmt::Display;

use derive_ctor::ctor;

use crate::{
    err::Error,
    structs::{DAEntry, DAHeader, DALoadRegion, Verify},
};

pub mod err;
mod structs;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(ctor)]
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

#[derive(ctor)]
pub struct DARegion {
    pub base: u32,
    pub code: Vec<u8>,
    pub is_signed: bool,
}

impl Display for DARegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Base address: {:#x}\n", self.base)?;
        write!(f, "Code length: {:#x}\n", self.code.len())?;
        write!(f, "Signed: {}\n", if self.is_signed { "yes" } else { "no" })
    }
}

pub fn parse(data: &[u8]) -> Result<Vec<DA>> {
    let config = bincode::config::standard()
        .with_little_endian()
        .with_fixed_int_encoding();
    let (da, bytes_read): (DAHeader, _) = bincode::decode_from_slice(data, config)?;
    da.verify()?;

    let mut vec = Vec::with_capacity(da.count as usize);
    for i in 0..da.count {
        let (da_entry, offset): (DAEntry, _) =
            bincode::decode_from_slice(&data[bytes_read + (i as usize * 0xdc)..], config)?;
        da_entry.verify()?;

        let mut regions = Vec::with_capacity(da_entry.region_count as usize);
        for j in 0..da_entry.region_count {
            let region: DALoadRegion = bincode::decode_from_slice(
                &data[bytes_read + (i as usize * 0xdc) + offset + (j as usize * 0x14)..],
                config,
            )?
            .0;
            region.verify()?;
            regions.push(DARegion::new(
                region.base,
                data[region.start as usize..(region.start + region.len) as usize].to_vec(),
                region.sig_len != 0,
            ));
        }

        vec.push(DA::new(
            da_entry.hw_code,
            da_entry.hw_subcode,
            da_entry.hw_version,
            da_entry.sw_version,
            regions,
        ))
    }

    Ok(vec)
}
