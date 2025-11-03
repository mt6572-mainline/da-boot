use thiserror::Error as TError;

#[derive(Debug, TError)]
pub enum Error {
    /// Invalid magic (MTK_DOWNLOAD_AGENT or 0x22668899 is not matched)
    #[error("Invalid magic")]
    InvalidMagic,
    /// Unexpected data
    #[error("Invalid struct data")]
    InvalidHeuristics,
    /// Invalid DA region count
    ///
    /// Raised when DA region count is 0
    #[error("Invalid DA region count")]
    InvalidRegionCount,
    /// Invalid DA code start position
    ///
    /// Raised when code offset is less than 0x100 from the DA start
    #[error("Invalid DA code start")]
    InvalidRegionStart,
    /// Invalid DA code size
    ///
    /// Raised when code size is less than 0x100
    #[error("Invalid DA code size")]
    InvalidCodeSize,

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// CStr decode error
    #[error("CStr decode error: {0}")]
    Cstr(#[from] std::ffi::FromBytesUntilNulError),

    /// bincode crate error
    #[error("Bincode decode error: {0}")]
    Bincode(#[from] bincode::error::DecodeError),

    /// Any other error
    #[error("{0}")]
    Custom(#[from] Box<dyn std::error::Error>),
}
