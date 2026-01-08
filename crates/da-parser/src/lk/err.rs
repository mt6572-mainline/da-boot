use thiserror::Error as TError;

#[derive(Debug, TError)]
pub enum Error {
    #[error("Invalid magic: {0:?}, expected 0x58881688")]
    InvalidHeaderMagic(u32),
    #[error("Invalid mode: {0}")]
    InvalidHeaderMode(u32),
}
