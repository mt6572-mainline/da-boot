use thiserror::Error as TError;

#[derive(Debug, TError)]
pub enum Error {
    /// `serialport` crate error
    #[error("serialport error: {0}")]
    SerialPort(#[from] serialport::Error),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
