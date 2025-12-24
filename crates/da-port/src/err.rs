use thiserror::Error as TError;

#[derive(Debug, TError)]
pub enum Error {
    #[cfg(feature = "serialport")]
    /// `serialport` crate error
    #[error("serialport error: {0}")]
    SerialPort(#[from] serialport::Error),

    #[cfg(feature = "std")]
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Any other error
    #[error("{0}")]
    Custom(#[from] Box<dyn core::error::Error>),
}
