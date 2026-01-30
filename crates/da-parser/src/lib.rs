use bincode::Decode;

use crate::err::Error;

pub mod da;
pub mod err;
pub mod lk;

pub type Result<T> = core::result::Result<T, Error>;

pub use da::hl::DA;
pub use lk::hl::LK;

pub trait LLParser: Decode<()> + Sized {
    type Error;

    fn parse(data: &[u8]) -> Result<Self> {
        let config = bincode::config::standard()
            .with_little_endian()
            .with_fixed_int_encoding();
        bincode::decode_from_slice(data, config)
            .map(|r| r.0)
            .map_err(|e| e.into())
    }
    fn validate(&self) -> core::result::Result<(), Self::Error>;
}

pub trait HLParser<T: LLParser>: Sized {
    fn parse(data: &[u8], position: usize, ll: T) -> Result<Self>;
    fn as_ll(&self) -> Result<T>;
}

pub fn parse_da(data: &[u8]) -> Result<DA> {
    DA::parse(
        data,
        size_of::<da::ll::Header>(),
        da::ll::Header::parse(data)?,
    )
}

pub fn parse_lk(data: &[u8]) -> Result<LK> {
    LK::parse(
        data,
        size_of::<lk::ll::Header>(),
        lk::ll::Header::parse(data)?,
    )
}

pub fn preloader_header_size(data: &[u8]) -> Result<usize> {
    if data.starts_with(b"EMMC_BOOT") {
        Ok(0xb00)
    } else if data.starts_with(b"MMM") {
        Ok(0x300)
    } else {
        Err(Error::UnknownPreloaderType)
    }
}
