use thiserror::Error as TError;

#[derive(Debug, TError)]
pub enum Error {
    /// DA parsing error
    #[error("DA parsing error: {0}")]
    DA(#[from] crate::da::err::Error),

    /// LK parsing error
    #[error("LK parsing error: {0}")]
    LK(#[from] crate::lk::err::Error),

    /// Unknown preloader type
    #[error("Unknown preloader type")]
    UnknownPreloaderType,

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CString creation error (from slice): {0}")]
    CString(#[from] std::array::TryFromSliceError),

    #[error("CString creation error (NUL): {0}")]
    NulError(#[from] std::ffi::NulError),

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
