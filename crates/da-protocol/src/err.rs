use thiserror::Error as TError;

/// Protocol errors
#[derive(Debug, TError)]
pub enum Error {
    /// postcard crate error
    #[error("postcard error: {0}")]
    Postcard(#[from] postcard::Error),
    /// `da-port` error
    #[error("da-port error: {0}")]
    DAPort(#[from] da_port::err::Error),
}
