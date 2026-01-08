use thiserror::Error as TError;

#[derive(Debug, TError)]
pub enum Error {
    #[error("Invalid magic: {0:?}, expected MTK_DOWNLOAD_AGENT")]
    InvalidHeaderMagic([u8; 18]),
    #[error("Invalid heuristics")]
    InvalidHeaderHeuristics,
    #[error("Invalid type: {0}, expected 0x22668899")]
    InvalidHeaderType(u32),

    #[error("Invalid magic: {0}, expected 0xDADA")]
    InvalidEntryMagic(u16),
    #[error("Invalid heuristics")]
    InvalidEntryHeuristics,

    #[error("Invalid region start: {0}, expected > 0x100")]
    InvalidRegionStart(u32),
    #[error("Invalid region size: {0}, expected > 0x1000")]
    InvalidRegionSize(u32),
    #[error("Invalid region base address: {0}")]
    InvalidRegionBase(u32),
}
