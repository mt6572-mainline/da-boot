use thiserror::Error as TError;

#[derive(Debug, TError)]
pub enum Error {
    #[cfg(feature = "std")]
    /// `serialport` crate error
    #[error("serialport error: {0}")]
    SerialPort(#[from] serialport::Error),

    #[cfg(feature = "std")]
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
